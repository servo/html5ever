// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod foreach_html5lib_test;

use foreach_html5lib_test::foreach_html5lib_test;
use html5ever::tendril::*;
use html5ever::tokenizer::states::{
    CdataSection, Data, Plaintext, RawData, Rawtext, Rcdata, ScriptData,
};
use html5ever::tokenizer::BufferQueue;
use html5ever::tokenizer::{CharacterTokens, EOFToken, NullCharacterToken, ParseError};
use html5ever::tokenizer::{CommentToken, DoctypeToken, TagToken, Token};
use html5ever::tokenizer::{Doctype, EndTag, StartTag, Tag};
use html5ever::tokenizer::{TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts};
use html5ever::{namespace_url, ns, Attribute, LocalName, QualName};
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::{char, env};

use util::runner::{run_all, Test};

mod util {
    pub mod runner;
}

#[derive(Debug)]
struct TestError;

impl PartialEq for TestError {
    fn eq(&self, _: &TestError) -> bool {
        // TODO: actually match exact error messages
        true
    }
}

// some large testcases hang forever without an upper-bound of splits to generate
const MAX_SPLITS: usize = 1000;

// Return all ways of splitting the string into at most n
// possibly-empty pieces.
fn splits(s: &str, n: usize) -> Vec<Vec<StrTendril>> {
    if n == 1 {
        return vec![vec![s.to_tendril()]];
    }

    let mut out = vec![];
    for p in s.char_indices().map(|(n, _)| n).chain(Some(s.len())) {
        let y = &s[p..];
        for mut x in splits(&s[..p], n - 1).into_iter() {
            x.push(y.to_tendril());
            out.push(x);
        }
    }

    out.extend(splits(s, n - 1));
    out.truncate(MAX_SPLITS);
    out
}

struct TokenLogger {
    tokens: RefCell<Vec<Token>>,
    errors: RefCell<Vec<TestError>>,
    current_str: RefCell<StrTendril>,
    exact_errors: bool,
}

impl TokenLogger {
    fn new(exact_errors: bool) -> TokenLogger {
        TokenLogger {
            tokens: RefCell::new(vec![]),
            errors: RefCell::new(vec![]),
            current_str: RefCell::new(StrTendril::new()),
            exact_errors,
        }
    }

    // Push anything other than character tokens
    fn push(&self, token: Token) {
        self.finish_str();
        self.tokens.borrow_mut().push(token);
    }

    fn finish_str(&self) {
        if self.current_str.borrow().len() > 0 {
            let s = self.current_str.take();
            self.tokens.borrow_mut().push(CharacterTokens(s));
        }
    }

    fn get_tokens(self) -> (Vec<Token>, Vec<TestError>) {
        self.finish_str();
        (self.tokens.take(), self.errors.take())
    }
}

impl TokenSink for TokenLogger {
    type Handle = ();

    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        match token {
            CharacterTokens(b) => {
                self.current_str.borrow_mut().push_slice(&b);
            },

            NullCharacterToken => {
                self.current_str.borrow_mut().push_char('\0');
            },

            ParseError(_) => {
                if self.exact_errors {
                    self.errors.borrow_mut().push(TestError);
                }
            },

            TagToken(mut t) => {
                // The spec seems to indicate that one can emit
                // erroneous end tags with attrs, but the test
                // cases don't contain them.
                match t.kind {
                    EndTag => {
                        t.self_closing = false;
                        t.attrs = vec![];
                    },
                    _ => t.attrs.sort_by(|a1, a2| a1.name.cmp(&a2.name)),
                }
                self.push(TagToken(t));
            },

            EOFToken => (),

            _ => self.push(token),
        }
        TokenSinkResult::Continue
    }
}

fn tokenize(input: Vec<StrTendril>, opts: TokenizerOpts) -> (Vec<Token>, Vec<TestError>) {
    let sink = TokenLogger::new(opts.exact_errors);
    let tok = Tokenizer::new(sink, opts);
    let buffer = BufferQueue::default();
    for chunk in input.into_iter() {
        buffer.push_back(chunk);
        let _ = tok.feed(&buffer);
    }
    let _ = tok.feed(&buffer);
    tok.end();
    tok.sink.get_tokens()
}

trait JsonExt: Sized {
    fn get_str(&self) -> String;
    fn get_tendril(&self) -> StrTendril;
    fn get_nullable_tendril(&self) -> Option<StrTendril>;
    fn get_bool(&self) -> bool;
    fn get_obj(&self) -> &Map<String, Self>;
    fn get_list(&self) -> &Vec<Self>;
    fn find(&self, key: &str) -> &Self;
}

impl JsonExt for Value {
    fn get_str(&self) -> String {
        match *self {
            Value::String(ref s) => s.to_string(),
            _ => panic!("Value::get_str: not a String"),
        }
    }

    fn get_tendril(&self) -> StrTendril {
        match *self {
            Value::String(ref s) => s.to_tendril(),
            _ => panic!("Value::get_tendril: not a String"),
        }
    }

    fn get_nullable_tendril(&self) -> Option<StrTendril> {
        match *self {
            Value::Null => None,
            Value::String(ref s) => Some(s.to_tendril()),
            _ => panic!("Value::get_nullable_tendril: not a String"),
        }
    }

    fn get_bool(&self) -> bool {
        match *self {
            Value::Bool(b) => b,
            _ => panic!("Value::get_bool: not a Bool"),
        }
    }

    fn get_obj(&self) -> &Map<String, Value> {
        match self {
            Value::Object(m) => m,
            _ => panic!("Value::get_obj: not an Object"),
        }
    }

    fn get_list(&self) -> &Vec<Value> {
        match self {
            Value::Array(m) => m,
            _ => panic!("Value::get_list: not an Array"),
        }
    }

    fn find(&self, key: &str) -> &Value {
        self.get_obj().get(key).unwrap()
    }
}

// Parse a JSON object (other than "ParseError") to a token.
fn json_to_token(js: &Value) -> Token {
    let parts = js.get_list();
    // Collect refs here so we don't have to use "ref" in all the patterns below.
    let args: Vec<&Value> = parts[1..].iter().collect();
    match &*parts[0].get_str() {
        "DOCTYPE" => DoctypeToken(Doctype {
            name: args[0].get_nullable_tendril(),
            public_id: args[1].get_nullable_tendril(),
            system_id: args[2].get_nullable_tendril(),
            force_quirks: !args[3].get_bool(),
        }),

        "StartTag" => TagToken(Tag {
            kind: StartTag,
            name: LocalName::from(&*args[0].get_str()),
            attrs: args[1]
                .get_obj()
                .iter()
                .map(|(k, v)| Attribute {
                    name: QualName::new(None, ns!(), LocalName::from(&**k)),
                    value: v.get_tendril(),
                })
                .collect(),
            self_closing: match args.get(2) {
                Some(b) => b.get_bool(),
                None => false,
            },
        }),

        "EndTag" => TagToken(Tag {
            kind: EndTag,
            name: LocalName::from(&*args[0].get_str()),
            attrs: vec![],
            self_closing: false,
        }),

        "Comment" => CommentToken(args[0].get_tendril()),

        "Character" => CharacterTokens(args[0].get_tendril()),

        // We don't need to produce NullCharacterToken because
        // the TokenLogger will convert them to CharacterTokens.
        _ => panic!("don't understand token {:?}", parts),
    }
}

// Parse the "output" field of the test case into a vector of tokens.
fn json_to_tokens(
    js_tokens: &Value,
    js_errors: &[Value],
    exact_errors: bool,
) -> (Vec<Token>, Vec<TestError>) {
    // Use a TokenLogger so that we combine character tokens separated
    // by an ignored error.
    let sink = TokenLogger::new(exact_errors);
    for tok in js_tokens.get_list().iter() {
        assert_eq!(
            sink.process_token(json_to_token(tok), 0),
            TokenSinkResult::Continue
        );
    }

    for err in js_errors {
        assert_eq!(
            sink.process_token(ParseError(err.find("code").get_str().into()), 0),
            TokenSinkResult::Continue
        );
    }

    sink.get_tokens()
}

// Undo the escaping in "doubleEscaped" tests.
fn unescape(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    loop {
        match it.next() {
            None => return Some(out),
            Some('\\') => {
                if it.peek() != Some(&'u') {
                    panic!("can't understand escape");
                }
                let _ = it.next();
                let hex: String = it.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    // Some of the tests use lone surrogates, but we have no
                    // way to represent them in the UTF-8 input to our parser.
                    // Since these can only come from script, we will catch
                    // them there.
                    None => return None,
                    Some(c) => out.push(c),
                }
            },
            Some(c) => out.push(c),
        }
    }
}

fn unescape_json(js: &Value) -> Value {
    match js {
        // unwrap is OK here because the spec'd *output* of the tokenizer never
        // contains a lone surrogate.
        Value::String(s) => Value::String(unescape(s).unwrap()),
        Value::Array(xs) => Value::Array(xs.iter().map(unescape_json).collect()),
        Value::Object(obj) => {
            let mut new_obj = Map::new();
            for (k, v) in obj.iter() {
                new_obj.insert(k.clone(), unescape_json(v));
            }
            Value::Object(new_obj)
        },
        _ => js.clone(),
    }
}

fn mk_test(
    desc: String,
    input: String,
    expect: Value,
    expect_errors: Vec<Value>,
    opts: TokenizerOpts,
) -> Test {
    Test {
        name: desc,
        skip: false,
        test: Box::new(move || {
            // Split up the input at different points to test incremental tokenization.
            let insplits = splits(&input, 3);
            for input in insplits.into_iter() {
                // Clone 'input' so we have it for the failure message.
                // Also clone opts.  If we don't, we get the wrong
                // result but the compiler doesn't catch it!
                // Possibly mozilla/rust#12223.
                let output = tokenize(input.clone(), opts.clone());
                let expect_toks = json_to_tokens(&expect, &expect_errors, opts.exact_errors);
                if output != expect_toks {
                    panic!(
                        "\ninput: {:?}\ngot: {:?}\nexpected: {:?}",
                        input, output, expect_toks
                    );
                }
            }
        }),
    }
}

fn mk_tests(tests: &mut Vec<Test>, filename: &str, js: &Value) {
    let obj = js.get_obj();
    let mut input = js.find("input").get_str();
    let mut expect = js.find("output").clone();
    let expect_errors = js
        .get("errors")
        .map(JsonExt::get_list)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let desc = format!("tok: {}: {}", filename, js.find("description").get_str());

    // "Double-escaped" tests require additional processing of
    // the input and output.
    if obj
        .get(&"doubleEscaped".to_string())
        .map_or(false, |j| j.get_bool())
    {
        match unescape(&input) {
            None => return,
            Some(i) => input = i,
        }
        expect = unescape_json(&expect);
    }

    // Some tests have a last start tag name.
    let start_tag = obj.get(&"lastStartTag".to_string()).map(|s| s.get_str());

    // Some tests want to start in a state other than Data.
    let state_overrides = match obj.get(&"initialStates".to_string()) {
        Some(Value::Array(xs)) => xs
            .iter()
            .map(|s| {
                Some(match &s.get_str()[..] {
                    "PLAINTEXT state" => Plaintext,
                    "RAWTEXT state" => RawData(Rawtext),
                    "RCDATA state" => RawData(Rcdata),
                    "Script data state" => RawData(ScriptData),
                    "CDATA section state" => CdataSection,
                    "Data state" => Data,
                    s => panic!("don't know state {}", s),
                })
            })
            .collect(),
        None => vec![None],
        _ => panic!("don't understand initialStates value"),
    };

    // Build the tests.
    for state in state_overrides.into_iter() {
        for &exact_errors in [false, true].iter() {
            let mut newdesc = desc.clone();
            if let Some(s) = state {
                newdesc = format!("{} (in state {:?})", newdesc, s)
            };
            if exact_errors {
                newdesc = format!("{} (exact errors)", newdesc);
            }

            tests.push(mk_test(
                newdesc,
                input.clone(),
                expect.clone(),
                expect_errors.to_owned(),
                TokenizerOpts {
                    exact_errors,
                    initial_state: state,
                    last_start_tag_name: start_tag.clone(),

                    // Not discarding a BOM is what the test suite expects; see
                    // https://github.com/html5lib/html5lib-tests/issues/2
                    discard_bom: false,

                    ..Default::default()
                },
            ));
        }
    }
}

fn tests(src_dir: &Path) -> Vec<Test> {
    let mut tests = vec![];

    let mut add_test = |path: &Path, mut file: File| {
        let mut s = String::new();
        file.read_to_string(&mut s).expect("file reading error");
        let js: Value = serde_json::from_str(&s).expect("json parse error");

        if let Some(Value::Array(lst)) = js.get_obj().get("tests") {
            for test in lst.iter() {
                mk_tests(
                    &mut tests,
                    path.file_name().unwrap().to_str().unwrap(),
                    test,
                )
            }
        }
    };

    foreach_html5lib_test(
        src_dir,
        "html5lib-tests/tokenizer",
        OsStr::new("test"),
        &mut add_test,
    );

    foreach_html5lib_test(
        src_dir,
        "custom-html5lib-tokenizer-tests",
        OsStr::new("test"),
        &mut add_test,
    );

    tests
}

fn main() {
    run_all(tests(Path::new(env!("CARGO_MANIFEST_DIR"))));
}
