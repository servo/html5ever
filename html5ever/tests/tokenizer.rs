// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate rustc_serialize;
extern crate rustc_test as test;
#[macro_use] extern crate html5ever;

mod foreach_html5lib_test;
use foreach_html5lib_test::foreach_html5lib_test;

use std::{char, env};
use std::ffi::OsStr;
use std::mem::replace;
use std::default::Default;
use std::path::Path;
use test::{TestDesc, TestDescAndFn, DynTestName, DynTestFn};
use rustc_serialize::json::Json;
use std::collections::BTreeMap;
use std::borrow::Cow::Borrowed;

use html5ever::{LocalName, QualName};
use html5ever::tokenizer::{Doctype, StartTag, EndTag, Tag};
use html5ever::tokenizer::{Token, DoctypeToken, TagToken, CommentToken};
use html5ever::tokenizer::{CharacterTokens, NullCharacterToken, EOFToken, ParseError};
use html5ever::tokenizer::{TokenSink, Tokenizer, TokenizerOpts, TokenSinkResult};
use html5ever::tokenizer::{BufferQueue};
use html5ever::tokenizer::states::{Plaintext, RawData, Rcdata, Rawtext};
use html5ever::tendril::*;
use html5ever::{Attribute};


// Return all ways of splitting the string into at most n
// possibly-empty pieces.
fn splits(s: &str, n: usize) -> Vec<Vec<StrTendril>> {
    if n == 1 {
        return vec!(vec!(s.to_tendril()));
    }

    let mut points: Vec<usize> = s.char_indices().map(|(n,_)| n).collect();
    points.push(s.len());

    // do this with iterators?
    let mut out = vec!();
    for p in points.into_iter() {
        let y = &s[p..];
        for mut x in splits(&s[..p], n-1).into_iter() {
            x.push(y.to_tendril());
            out.push(x);
        }
    }

    out.extend(splits(s, n-1).into_iter());
    out
}

struct TokenLogger {
    tokens: Vec<Token>,
    current_str: StrTendril,
    exact_errors: bool,
}

impl TokenLogger {
    fn new(exact_errors: bool) -> TokenLogger {
        TokenLogger {
            tokens: vec!(),
            current_str: StrTendril::new(),
            exact_errors: exact_errors,
        }
    }

    // Push anything other than character tokens
    fn push(&mut self, token: Token) {
        self.finish_str();
        self.tokens.push(token);
    }

    fn finish_str(&mut self) {
        if self.current_str.len() > 0 {
            let s = replace(&mut self.current_str, StrTendril::new());
            self.tokens.push(CharacterTokens(s));
        }
    }

    fn get_tokens(mut self) -> Vec<Token> {
        self.finish_str();
        self.tokens
    }
}

impl TokenSink for TokenLogger {
    type Handle = ();

    fn process_token(&mut self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        match token {
            CharacterTokens(b) => {
                self.current_str.push_slice(&b);
            }

            NullCharacterToken => {
                self.current_str.push_char('\0');
            }

            ParseError(_) => if self.exact_errors {
                self.push(ParseError(Borrowed("")));
            },

            TagToken(mut t) => {
                // The spec seems to indicate that one can emit
                // erroneous end tags with attrs, but the test
                // cases don't contain them.
                match t.kind {
                    EndTag => {
                        t.self_closing = false;
                        t.attrs = vec!();
                    }
                    _ => t.attrs.sort_by(|a1, a2| a1.name.cmp(&a2.name)),
                }
                self.push(TagToken(t));
            }

            EOFToken => (),

            _ => self.push(token),
        }
        TokenSinkResult::Continue
    }
}

fn tokenize(input: Vec<StrTendril>, opts: TokenizerOpts) -> Vec<Token> {
    let sink = TokenLogger::new(opts.exact_errors);
    let mut tok = Tokenizer::new(sink, opts);
    let mut buffer = BufferQueue::new();
    for chunk in input.into_iter() {
        buffer.push_back(chunk);
        let _ = tok.feed(&mut buffer);
    }
    let _ = tok.feed(&mut buffer);
    tok.end();
    tok.sink.get_tokens()
}

trait JsonExt: Sized {
    fn get_str(&self) -> String;
    fn get_tendril(&self) -> StrTendril;
    fn get_nullable_tendril(&self) -> Option<StrTendril>;
    fn get_bool(&self) -> bool;
    fn get_obj<'t>(&'t self) -> &'t BTreeMap<String, Self>;
    fn get_list<'t>(&'t self) -> &'t Vec<Self>;
    fn find<'t>(&'t self, key: &str) -> &'t Self;
}

impl JsonExt for Json {
    fn get_str(&self) -> String {
        match *self {
            Json::String(ref s) => s.to_string(),
            _ => panic!("Json::get_str: not a String"),
        }
    }

    fn get_tendril(&self) -> StrTendril {
        match *self {
            Json::String(ref s) => s.to_tendril(),
            _ => panic!("Json::get_tendril: not a String"),
        }
    }

    fn get_nullable_tendril(&self) -> Option<StrTendril> {
        match *self {
            Json::Null => None,
            Json::String(ref s) => Some(s.to_tendril()),
            _ => panic!("Json::get_nullable_tendril: not a String"),
        }
    }

    fn get_bool(&self) -> bool {
        match *self {
            Json::Boolean(b) => b,
            _ => panic!("Json::get_bool: not a Boolean"),
        }
    }

    fn get_obj<'t>(&'t self) -> &'t BTreeMap<String, Json> {
        match *self {
            Json::Object(ref m) => &*m,
            _ => panic!("Json::get_obj: not an Object"),
        }
    }

    fn get_list<'t>(&'t self) -> &'t Vec<Json> {
        match *self {
            Json::Array(ref m) => m,
            _ => panic!("Json::get_list: not an Array"),
        }
    }

    fn find<'t>(&'t self, key: &str) -> &'t Json {
        self.get_obj().get(&key.to_string()).unwrap()
    }
}

// Parse a JSON object (other than "ParseError") to a token.
fn json_to_token(js: &Json) -> Token {
    let parts = js.get_list();
    // Collect refs here so we don't have to use "ref" in all the patterns below.
    let args: Vec<&Json> = parts[1..].iter().collect();
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
            attrs: args[1].get_obj().iter().map(|(k,v)| {
                Attribute {
                    name: QualName::new(None, ns!(), LocalName::from(&**k)),
                    value: v.get_tendril()
                }
            }).collect(),
            self_closing: match args.get(2) {
                Some(b) => b.get_bool(),
                None => false,
            }
        }),

        "EndTag" => TagToken(Tag {
            kind: EndTag,
            name: LocalName::from(&*args[0].get_str()),
            attrs: vec!(),
            self_closing: false
        }),

        "Comment" => CommentToken(args[0].get_tendril()),

        "Character" => CharacterTokens(args[0].get_tendril()),

        // We don't need to produce NullCharacterToken because
        // the TokenLogger will convert them to CharacterTokens.

        _ => panic!("don't understand token {:?}", parts),
    }
}

// Parse the "output" field of the test case into a vector of tokens.
fn json_to_tokens(js: &Json, exact_errors: bool) -> Vec<Token> {
    // Use a TokenLogger so that we combine character tokens separated
    // by an ignored error.
    let mut sink = TokenLogger::new(exact_errors);
    for tok in js.get_list().iter() {
        assert_eq!(match *tok {
            Json::String(ref s)
                if &s[..] == "ParseError" => sink.process_token(ParseError(Borrowed("")), 0),
            _ => sink.process_token(json_to_token(tok), 0),
        }, TokenSinkResult::Continue);
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
                drop(it.next());
                let hex: String = it.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok()
                          .and_then(char::from_u32) {
                    // Some of the tests use lone surrogates, but we have no
                    // way to represent them in the UTF-8 input to our parser.
                    // Since these can only come from script, we will catch
                    // them there.
                    None => return None,
                    Some(c) => out.push(c),
                }
            }
            Some(c) => out.push(c),
        }
    }
}

fn unescape_json(js: &Json) -> Json {
    match *js {
        // unwrap is OK here because the spec'd *output* of the tokenizer never
        // contains a lone surrogate.
        Json::String(ref s) => Json::String(unescape(&s).unwrap()),
        Json::Array(ref xs) => Json::Array(xs.iter().map(unescape_json).collect()),
        Json::Object(ref obj) => {
            let mut new_obj = BTreeMap::new();
            for (k,v) in obj.iter() {
                new_obj.insert(k.clone(), unescape_json(v));
            }
            Json::Object(new_obj)
        }
        _ => js.clone(),
    }
}

fn mk_test(desc: String, input: String, expect: Json, opts: TokenizerOpts)
        -> TestDescAndFn {
    TestDescAndFn {
        desc: TestDesc::new(DynTestName(desc)),
        testfn: DynTestFn(Box::new(move || {
            // Split up the input at different points to test incremental tokenization.
            let insplits = splits(&input, 3);
            for input in insplits.into_iter() {
                // Clone 'input' so we have it for the failure message.
                // Also clone opts.  If we don't, we get the wrong
                // result but the compiler doesn't catch it!
                // Possibly mozilla/rust#12223.
                let output = tokenize(input.clone(), opts.clone());
                let expect_toks = json_to_tokens(&expect, opts.exact_errors);
                if output != expect_toks {
                    panic!("\ninput: {:?}\ngot: {:?}\nexpected: {:?}",
                        input, output, expect);
                }
            }
        })),
    }
}

fn mk_tests(tests: &mut Vec<TestDescAndFn>, filename: &str, js: &Json) {
    let obj = js.get_obj();
    let mut input = js.find("input").unwrap().get_str();
    let mut expect = js.find("output").unwrap().clone();
    let desc = format!("tok: {}: {}",
        filename, js.find("description").unwrap().get_str());

    // "Double-escaped" tests require additional processing of
    // the input and output.
    if obj.get(&"doubleEscaped".to_string()).map_or(false, |j| j.get_bool()) {
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
        Some(&Json::Array(ref xs)) => xs.iter().map(|s|
            Some(match &s.get_str()[..] {
                "PLAINTEXT state" => Plaintext,
                "RAWTEXT state"   => RawData(Rawtext),
                "RCDATA state"    => RawData(Rcdata),
                s => panic!("don't know state {}", s),
            })).collect(),
        None => vec!(None),
        _ => panic!("don't understand initialStates value"),
    };

    // Build the tests.
    for state in state_overrides.into_iter() {
        for &exact_errors in [false, true].iter() {
            let mut newdesc = desc.clone();
            match state {
                Some(s) => newdesc = format!("{} (in state {:?})", newdesc, s),
                None  => (),
            };
            if exact_errors {
                newdesc = format!("{} (exact errors)", newdesc);
            }

            tests.push(mk_test(newdesc, input.clone(), expect.clone(), TokenizerOpts {
                exact_errors: exact_errors,
                initial_state: state,
                last_start_tag_name: start_tag.clone(),

                // Not discarding a BOM is what the test suite expects; see
                // https://github.com/html5lib/html5lib-tests/issues/2
                discard_bom: false,

                .. Default::default()
            }));
        }
    }
}

fn tests(src_dir: &Path) -> Vec<TestDescAndFn> {
    let mut tests = vec!();

    foreach_html5lib_test(src_dir, "tokenizer",
                          OsStr::new("test"), |path, mut file| {
        let js = Json::from_reader(&mut file).ok().expect("json parse error");

        match js.get_obj().get(&"tests".to_string()) {
            Some(&Json::Array(ref lst)) => {
                for test in lst.iter() {
                    mk_tests(&mut tests, path.file_name().unwrap().to_str().unwrap(), test);
                }
            }

            // xmlViolation.test doesn't follow this format.
            _ => (),
        }
    });

    tests
}

fn main() {
    let args: Vec<_> = env::args().collect();
    test::test_main(&args, tests(Path::new(env!("CARGO_MANIFEST_DIR"))));
}
