/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::io;
use std::os;
use std::mem::replace;
use test::{TestDesc, TestDescAndFn, DynTestName, DynTestFn, test_main};
use extra::json;
use extra::json::{Json, ToJson};
use collections::treemap::TreeMap;

use html5::tokenizer::{Doctype, Attribute, StartTag, EndTag, Tag, Token};
use html5::tokenizer::{DoctypeToken, TagToken, CommentToken};
use html5::tokenizer::{CharacterToken, EOFToken, ParseError};
use html5::tokenizer::{TokenSink, Tokenizer};

struct TokenLogger {
    tokens: ~[Json],
    current_str: ~str,
}

impl TokenLogger {
    fn new() -> TokenLogger {
        TokenLogger {
            tokens: ~[],
            current_str: ~"",
        }
    }

    // Anything but Character
    fn push<T: json::ToJson>(&mut self, val: T) {
        self.finish_str();
        self.tokens.push(val.to_json());
    }

    fn finish_str(&mut self) {
        if self.current_str.len() > 0 {
            let s = replace(&mut self.current_str, ~"");
            self.tokens.push((~[~"Character", s]).to_json());
        }
    }
}

impl TokenSink for TokenLogger {
    fn process_token(&mut self, token: Token) {
        match token {
            CharacterToken(c) => {
                self.current_str.push_char(c);
            }

            TagToken(Tag { kind: StartTag, name, self_closing, attrs }) => {
                let mut attrmap = TreeMap::new();
                for Attribute { name, value } in attrs.move_iter() {
                    attrmap.insert(name, value.to_json());
                }

                let mut out = ~[(~"StartTag").to_json(), name.to_json(), attrmap.to_json()];
                if self_closing {
                    out.push(true.to_json());
                }
                self.push(out);
            }

            TagToken(Tag { kind: EndTag, name, .. }) => {
                self.push(~[~"EndTag", name]);
            }

            DoctypeToken(Doctype { name, public_id, system_id, force_quirks }) => {
                self.push(~[(~"DOCTYPE").to_json(),
                    name.to_json(), public_id.to_json(), system_id.to_json(),
                    (!force_quirks).to_json()]);
            }

            CommentToken(s) => self.push(~[~"Comment", s]),

            ParseError(_) => self.push(~"ParseError"),

            EOFToken => (),
        }
    }
}

fn tokenize_to_json(input: ~str) -> Json {
    let mut sink = TokenLogger::new();
    {
        let mut tok = Tokenizer::new(&mut sink);
        tok.feed(input);
        tok.end();
    }
    sink.finish_str();
    sink.tokens.to_json()
}

trait JsonExt {
    fn get_str(&self) -> ~str;
    fn get_bool(&self) -> bool;
    fn get_obj<'t>(&'t self) -> &'t TreeMap<~str, Self>;
    fn find<'t>(&'t self, key: &str) -> &'t Self;
}

impl JsonExt for Json {
    fn get_str(&self) -> ~str {
        match *self {
            json::String(ref s) => s.clone(),
            _ => fail!("Json::get_str: not a String"),
        }
    }

    fn get_bool(&self) -> bool {
        match *self {
            json::Boolean(b) => b,
            _ => fail!("Json::get_bool: not a Boolean"),
        }
    }

    fn get_obj<'t>(&'t self) -> &'t TreeMap<~str, Json> {
        match *self {
            json::Object(ref m) => &**m,
            _ => fail!("Json::get_obj: not an Object"),
        }
    }

    fn find<'t>(&'t self, key: &str) -> &'t Json {
        self.get_obj().find(&key.to_owned()).unwrap()
    }
}

fn mk_test(path_str: &str, js: &Json) -> Option<TestDescAndFn> {
    let input  = js.find("input").get_str();
    let expect = js.find("output").clone();
    let desc = format!("{:s}: {:s}",
        path_str, js.find("description").get_str());

    // "Double-escaped" tests require additional processing of
    // the input and output.
    let obj = js.get_obj();
    let double_esc = obj.find(&~"doubleEscaped")
        .map_or(false, |j| j.get_bool());
    if double_esc {
        // FIXME: implement this
        return None;
    }

    // Some tests want to start in a state other than Data.
    if obj.find(&~"initialStates").is_some() {
        // FIXME: We can't handle that either
        return None;
    }

    Some(TestDescAndFn {
        desc: TestDesc {
            name: DynTestName(desc),
            ignore: false,
            should_fail: false,
        },
        testfn: DynTestFn(proc() {
            // Clone so we still have 'input' for the failure message.
            let output = tokenize_to_json(input.clone());
            if output != expect {
                fail!("\ninput: {:?}\ngot: {:s}\nexpected: {:s}",
                    input, output.to_pretty_str(), expect.to_pretty_str());
            }
        }),
    })
}

static test_blacklist: &'static [&'static str] = &[
    "xmlViolation.test",
];

pub fn run_tests() {
    let mut tests: ~[TestDescAndFn] = ~[];

    let test_dir_path = FromStr::from_str("test-json/tokenizer").unwrap();
    let test_files = io::fs::readdir(&test_dir_path).ok().expect("can't open dir");

    for path in test_files.move_iter() {
        let path_str = path.filename_str().unwrap();

        if !path_str.ends_with(".test") { continue; }
        if test_blacklist.iter().any(|&t| t == path_str) { continue; }

        let warn_skip = || {
            println!("WARNING: can't load {:s}", path_str);
        };

        let mut file = io::File::open(&path).ok().expect("can't open file");
        let js = match json::from_reader(&mut file as &mut Reader) {
            Err(_) => { warn_skip(); continue; }
            Ok(j) => j,
        };

        match *js.find("tests") {
            json::List(ref lst) => {
                for test in lst.iter() {
                    match mk_test(path_str.as_slice(), test) {
                        Some(t) => tests.push(t),
                        None => (),
                    }
                }
            }
            _ => warn_skip(),
        }
    }

    let args = os::args();
    test_main(args.as_slice(), tests);
}
