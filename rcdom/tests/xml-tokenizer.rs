// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use serde_json::{Map, Value};
use std::borrow::Cow::Borrowed;
use std::cell::RefCell;
use std::env;
use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;

use util::find_tests::foreach_xml5lib_test;
use util::runner::{run_all, Test};

use markup5ever::buffer_queue::BufferQueue;
use xml5ever::tendril::{SliceExt, StrTendril};
use xml5ever::tokenizer::{CharacterTokens, Token, TokenSink};
use xml5ever::tokenizer::{CommentToken, EmptyTag, EndTag, ShortTag, StartTag, Tag};
use xml5ever::tokenizer::{Doctype, DoctypeToken, PIToken, Pi};
use xml5ever::tokenizer::{EOFToken, XmlTokenizer, XmlTokenizerOpts};
use xml5ever::tokenizer::{NullCharacterToken, ParseError, TagToken};
use xml5ever::{namespace_url, ns, Attribute, LocalName, QualName};

mod util {
    pub mod find_tests;
    pub mod runner;
}

// Return all ways of splitting the string into at most n
// possibly-empty pieces.
fn splits(s: &str, n: usize) -> Vec<Vec<StrTendril>> {
    if n == 1 {
        return vec![vec![s.to_tendril()]];
    }

    let mut points: Vec<usize> = s.char_indices().map(|(n, _)| n).collect();
    points.push(s.len());

    // do this with iterators?
    let mut out = vec![];
    for p in points.into_iter() {
        let y = &s[p..];
        for mut x in splits(&s[..p], n - 1).into_iter() {
            x.push(y.to_tendril());
            out.push(x);
        }
    }

    out.extend(splits(s, n - 1));
    out
}

struct TokenLogger {
    tokens: RefCell<Vec<Token>>,
    current_str: RefCell<StrTendril>,
    exact_errors: bool,
}

impl TokenLogger {
    fn new(exact_errors: bool) -> TokenLogger {
        TokenLogger {
            tokens: RefCell::new(vec![]),
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

    fn get_tokens(self) -> Vec<Token> {
        self.finish_str();
        self.tokens.take()
    }
}

impl TokenSink for TokenLogger {
    fn process_token(&self, token: Token) {
        match token {
            CharacterTokens(b) => {
                self.current_str.borrow_mut().push_slice(&b);
            },

            NullCharacterToken => {
                self.current_str.borrow_mut().push_char('\0');
            },

            ParseError(_) => {
                if self.exact_errors {
                    self.push(ParseError(Borrowed("")));
                }
            },

            TagToken(mut t) => {
                // The spec seems to indicate that one can emit
                // erroneous end tags with attrs, but the test
                // cases don't contain them.
                match t.kind {
                    EndTag => {
                        t.attrs = vec![];
                    },
                    _ => t.attrs.sort_by(|a1, a2| a1.name.cmp(&a2.name)),
                }
                self.push(TagToken(t));
            },

            EOFToken => (),

            _ => self.push(token),
        }
    }
}

fn tokenize_xml(input: Vec<StrTendril>, opts: XmlTokenizerOpts) -> Vec<Token> {
    let sink = TokenLogger::new(opts.exact_errors);
    let tok = XmlTokenizer::new(sink, opts);
    let buf = BufferQueue::default();

    for chunk in input.into_iter() {
        buf.push_back(chunk);
        tok.feed(&buf);
    }
    tok.feed(&buf);
    tok.end();
    tok.sink.get_tokens()
}

#[allow(dead_code)]
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
            _ => panic!("Value::get_bool: not a Boolean"),
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
        self.get_obj().get(&key.to_string()).unwrap()
    }
}

// Parse a JSON object (other than "ParseError") to a token.
fn json_to_token(js: &Value) -> Token {
    let parts = js.as_array().unwrap();
    // Collect refs here so we don't have to use "ref" in all the patterns below.
    let args: Vec<&Value> = parts[1..].iter().collect();
    match &*parts[0].get_str() {
        "StartTag" => TagToken(Tag {
            kind: StartTag,
            name: QualName::new(None, ns!(), LocalName::from(args[0].get_str())),
            attrs: args[1]
                .get_obj()
                .iter()
                .map(|(k, v)| Attribute {
                    name: QualName::new(None, ns!(), LocalName::from(&**k)),
                    value: v.get_tendril(),
                })
                .collect(),
        }),

        "EndTag" => TagToken(Tag {
            kind: EndTag,
            name: QualName::new(None, ns!(), LocalName::from(args[0].get_str())),
            attrs: vec![],
        }),

        "ShortTag" => TagToken(Tag {
            kind: ShortTag,
            name: QualName::new(None, ns!(), LocalName::from(args[0].get_str())),
            attrs: vec![],
        }),

        "EmptyTag" => TagToken(Tag {
            kind: EmptyTag,
            name: QualName::new(None, ns!(), LocalName::from(args[0].get_str())),
            attrs: args[1]
                .get_obj()
                .iter()
                .map(|(k, v)| Attribute {
                    name: QualName::new(None, ns!(), LocalName::from(&**k)),
                    value: v.get_tendril(),
                })
                .collect(),
        }),

        "Comment" => CommentToken(args[0].get_tendril()),

        "Character" => CharacterTokens(args[0].get_tendril()),

        "PI" => PIToken(Pi {
            target: args[0].get_tendril(),
            data: args[1].get_tendril(),
        }),

        "DOCTYPE" => DoctypeToken(Doctype {
            name: args[0].get_nullable_tendril(),
            public_id: args[1].get_nullable_tendril(),
            system_id: args[2].get_nullable_tendril(),
        }),

        // We don't need to produce NullCharacterToken because
        // the TokenLogger will convert them to CharacterTokens.
        _ => panic!("don't understand token {:?}", parts),
    }
}

// Parse the "output" field of the test case into a vector of tokens.
fn json_to_tokens(js: &Value, exact_errors: bool) -> Vec<Token> {
    // Use a TokenLogger so that we combine character tokens separated
    // by an ignored error.
    let sink = TokenLogger::new(exact_errors);
    for tok in js.as_array().unwrap().iter() {
        match *tok {
            Value::String(ref s) if &s[..] == "ParseError" => {
                sink.process_token(ParseError(Borrowed("")))
            },
            _ => sink.process_token(json_to_token(tok)),
        }
    }
    sink.get_tokens()
}

fn mk_xml_test(name: String, input: String, expect: Value, opts: XmlTokenizerOpts) -> Test {
    Test {
        name,
        skip: false,
        test: Box::new(move || {
            // Split up the input at different points to test incremental tokenization.
            let insplits = splits(&input, 3);
            for input in insplits.into_iter() {
                // Clone 'input' so we have it for the failure message.
                // Also clone opts.  If we don't, we get the wrong
                // result but the compiler doesn't catch it!
                // Possibly mozilla/rust#12223.
                let output = tokenize_xml(input.clone(), opts);
                let expect = json_to_tokens(&expect, opts.exact_errors);
                if output != expect {
                    panic!(
                        "\ninput: {:?}\ngot: {:?}\nexpected: {:?}",
                        input, output, expect
                    );
                }
            }
        }),
    }
}

fn mk_xml_tests(tests: &mut Vec<Test>, filename: &str, js: &Value) {
    let input: &str = &js.find("input").get_str();
    let expect = js.find("output");
    let desc = format!("tok: {}: {}", filename, js.find("description").get_str());

    // Some tests want to start in a state other than Data.
    let state_overrides = vec![None];

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

            tests.push(mk_xml_test(
                newdesc,
                String::from(input),
                expect.clone(),
                XmlTokenizerOpts {
                    exact_errors,
                    initial_state: state,

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
    foreach_xml5lib_test(
        src_dir,
        "tokenizer",
        OsStr::new("test"),
        |path, mut file| {
            let mut s = String::new();
            file.read_to_string(&mut s).expect("file reading error");
            let js: Value = serde_json::from_str(&s).expect("json parse error");

            if let Value::Array(ref lst) = js["tests"] {
                for test in lst.iter() {
                    mk_xml_tests(
                        &mut tests,
                        path.file_name().unwrap().to_str().unwrap(),
                        test,
                    );
                }
            }
        },
    );

    tests
}

fn main() {
    run_all(tests(Path::new(env!("CARGO_MANIFEST_DIR"))));
}
