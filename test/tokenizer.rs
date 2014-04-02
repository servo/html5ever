/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::{io, str, num, char};
use std::mem::replace;
use test::{TestDesc, TestDescAndFn, DynTestName, DynTestFn};
use extra::json;
use extra::json::{Json, ToJson};
use collections::treemap::TreeMap;

use html5::tokenizer::{Doctype, Attribute, StartTag, EndTag, Tag, Token};
use html5::tokenizer::{DoctypeToken, TagToken, CommentToken};
use html5::tokenizer::{CharacterToken, MultiCharacterToken, EOFToken, ParseError};
use html5::tokenizer::{TokenSink, Tokenizer};
use html5::tokenizer::states::{State, Plaintext, RawData, Rcdata, Rawtext};

// Return all ways of splitting the string into at most n
// possibly-empty pieces.
fn splits(s: &str, n: uint) -> ~[~[~str]] {
    if n == 1 {
        return ~[~[s.to_owned()]];
    }

    let mut points: ~[uint] = s.char_indices().map(|(n,_)| n).collect();
    points.push(s.len());

    // do this with iterators?
    let mut out = ~[];
    for p in points.move_iter() {
        let y = s.slice_from(p);
        for mut x in splits(s.slice_to(p), n-1).move_iter() {
            x.push(y.to_owned());
            out.push(x);
        }
    }

    out.push_all_move(splits(s, n-1));
    out
}

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

            MultiCharacterToken(b) => {
                self.current_str.push_str(b);
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

fn tokenize_to_json(input: ~[~str], state: Option<State>, start_tag: Option<~str>) -> Json {
    let mut sink = TokenLogger::new();
    {
        let mut tok = Tokenizer::new(&mut sink);
        match state {
            Some(s) => tok.set_state(s),
            None => (),
        }
        if start_tag.is_some() {
            tok.set_last_start_tag_name(start_tag);
        }
        for chunk in input.move_iter() {
            tok.feed(chunk);
        }
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

// Undo the escaping in "doubleEscaped" tests.
fn unescape(s: &str) -> Option<~str> {
    let mut out = str::with_capacity(s.len());
    let mut it = s.chars().peekable();
    loop {
        match it.next() {
            None => return Some(out),
            Some('\\') if it.peek() == Some(&'u') => {
                drop(it.next());
                let hex: ~str = it.by_ref().take(4).collect();
                match num::from_str_radix(hex.as_slice(), 16)
                          .and_then(char::from_u32) {
                    // Some of the tests use lone surrogates, but we have no
                    // way to represent them in the UTF-8 input to our parser.
                    // Since these can only come from script, we will catch
                    // them there.
                    None => return None,
                    Some(c) => out.push_char(c),
                }
            }
            Some('\\') => fail!("can't understand escape"),
            Some(c) => out.push_char(c),
        }
    }
}

fn unescape_json(js: &Json) -> Json {
    match *js {
        // unwrap is OK here because the spec'd *output* of the tokenizer never
        // contains a lone surrogate.
        json::String(ref s) => json::String(unescape(s.as_slice()).unwrap()),
        json::List(ref xs) => json::List(xs.iter().map(unescape_json).collect()),
        json::Object(ref obj) => {
            let mut new_obj = ~TreeMap::new();
            for (k,v) in obj.iter() {
                new_obj.insert(k.clone(), unescape_json(v));
            }
            json::Object(new_obj)
        }
        _ => js.clone(),
    }
}

fn mk_test(desc: ~str, insplits: ~[~[~str]], expect: Json,
    state: Option<State>, start_tag: Option<~str>) -> TestDescAndFn {
    TestDescAndFn {
        desc: TestDesc {
            name: DynTestName(desc),
            ignore: false,
            should_fail: false,
        },
        testfn: DynTestFn(proc() {
            for input in insplits.move_iter() {
                // Clone 'input' so we have it for the failure message.
                // Also clone start_tag.  If we don't, we get the wrong
                // result but the compiler doesn't catch it!
                // Possibly mozilla/rust#12223.
                let output = tokenize_to_json(
                    input.clone(), state.clone(), start_tag.clone());
                if output != expect {
                    fail!("\ninput: {:?}\ngot: {:s}\nexpected: {:s}",
                        input, output.to_pretty_str(), expect.to_pretty_str());
                }
            }
        }),
    }
}

fn mk_tests(tests: &mut ~[TestDescAndFn], path_str: &str, js: &Json) {
    let obj = js.get_obj();
    let mut input = js.find("input").get_str();
    let mut expect = js.find("output").clone();
    let desc = format!("{:s}: {:s}",
        path_str, js.find("description").get_str());

    // "Double-escaped" tests require additional processing of
    // the input and output.
    if obj.find(&~"doubleEscaped").map_or(false, |j| j.get_bool()) {
        match unescape(input.as_slice()) {
            None => return,
            Some(i) => input = i,
        }
        expect = unescape_json(&expect);
    }

    if input.starts_with("\ufeff") {
        // The tests assume the BOM will pass through because they model
        // data sent from JavaScript rather than from a decoded document.
        // https://github.com/html5lib/html5lib-tests/issues/2
        return;
    }

    // Split up the input at different points to test incremental tokenization.
    let insplits = splits(input, 3);

    // Some tests have a last start tag name.
    let start_tag = obj.find(&~"lastStartTag").map(|s| s.get_str());

    // Some tests want to start in a state other than Data.
    match obj.find(&~"initialStates") {
        Some(&json::List(ref xs)) => for x in xs.iter() {
            let statestr = x.get_str();
            let state = match statestr.as_slice() {
                "PLAINTEXT state" => Plaintext,
                "RAWTEXT state"   => RawData(Rawtext),
                "RCDATA state"    => RawData(Rcdata),
                s => fail!("don't know state {:?}", s),
            };
            let newdesc = format!("{:s} (in {:s})", desc, statestr);
            tests.push(mk_test(newdesc, insplits.clone(), expect.clone(),
                Some(state), start_tag.clone()));
        },
        _ => tests.push(mk_test(desc, insplits, expect, None, start_tag)),
    }
}

pub fn tests() -> ~[TestDescAndFn] {
    let mut tests: ~[TestDescAndFn] = ~[];

    let test_dir_path = FromStr::from_str("test-json/tokenizer").unwrap();
    let test_files = io::fs::readdir(&test_dir_path).ok().expect("can't open dir");

    for path in test_files.move_iter() {
        let path_str = path.filename_str().unwrap();
        if !path_str.ends_with(".test") { continue; }

        let mut file = io::File::open(&path).ok().expect("can't open file");
        let js = json::from_reader(&mut file as &mut Reader)
            .ok().expect("json parse error");

        match js.get_obj().find(&~"tests") {
            Some(&json::List(ref lst)) => {
                for test in lst.iter() {
                    mk_tests(&mut tests, path_str.as_slice(), test);
                }
            }

            // xmlViolation.test doesn't follow this format.
            _ => (),
        }
    }

    tests
}
