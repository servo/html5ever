#![feature(test, plugin, slice_patterns, start, rt)]
#![plugin(string_cache_plugin)]

extern crate rustc_serialize;
extern crate string_cache;
extern crate tendril;
extern crate test;
extern crate xml5ever;

use std::env;
use std::borrow::Cow::Borrowed;
use std::ffi::OsStr;
use std::mem::replace;
use std::path::Path;
use std::rt;
use rustc_serialize::json::Json;
use string_cache::{Atom, QualName};
use tendril::{StrTendril, SliceExt};
use test::{TestDesc, TestDescAndFn, DynTestName, DynTestFn, ShouldPanic};
use util::find_tests::foreach_xml5lib_test;

use xml5ever::tokenizer::{Attribute};
use xml5ever::tokenizer::{XTag, StartXTag, EndXTag, CommentXToken, EmptyXTag, ShortXTag};
use xml5ever::tokenizer::{XToken, CharacterXTokens, XTokenSink};
use xml5ever::tokenizer::{NullCharacterXToken, XParseError, XTagToken};
use xml5ever::tokenizer::{PIToken, XPi};
use xml5ever::tokenizer::{EOFXToken, XmlTokenizer, XmlTokenizerOpts};

mod util { 
    pub mod find_tests;
}

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

struct XTokenLogger {
    tokens: Vec<XToken>,
    current_str: StrTendril,
    exact_errors: bool,
}


impl XTokenLogger {
    fn new(exact_errors: bool) -> XTokenLogger {
        XTokenLogger {
            tokens: vec!(),
            current_str: StrTendril::new(),
            exact_errors: exact_errors,
        }
    }

    // Push anything other than character tokens
    fn push(&mut self, token: XToken) {
        self.finish_str();
        self.tokens.push(token);
    }

    fn finish_str(&mut self) {
        if self.current_str.len() > 0 {
            let s = replace(&mut self.current_str, StrTendril::new());
            self.tokens.push(CharacterXTokens(s));
        }
    }

    fn get_tokens(mut self) -> Vec<XToken> {
        self.finish_str();
        self.tokens
    }
}

impl XTokenSink for XTokenLogger {
    fn process_token(&mut self, token: XToken) {
        match token {
            CharacterXTokens(b) => {
                self.current_str.push_slice(&b);
            }

            NullCharacterXToken => {
                self.current_str.push_char('\0');
            }

            XParseError(_) => if self.exact_errors {
                self.push(XParseError(Borrowed("")));
            },

            XTagToken(mut t) => {
                // The spec seems to indicate that one can emit
                // erroneous end tags with attrs, but the test
                // cases don't contain them.
                match t.kind {
                    EndXTag => {
                        t.attrs = vec!();
                    }
                    _ => t.attrs.sort_by(|a1, a2| a1.name.cmp(&a2.name)),
                }
                self.push(XTagToken(t));
            }

            EOFXToken => (),

            _ => self.push(token),
        }
    }
}

fn tokenize_xml(input: Vec<StrTendril>, opts: XmlTokenizerOpts) -> Vec<XToken> {
    let sink = XTokenLogger::new(opts.exact_errors);
    let mut tok = XmlTokenizer::new(sink, opts);
    for chunk in input.into_iter() {
        tok.feed(chunk);
    }
    tok.end();
    tok.unwrap().get_tokens()
}

// Parse a JSON object (other than "ParseError") to a token.
fn json_to_xtoken(js: &Json) -> XToken {
    let parts = js.as_array().unwrap();
    // Collect refs here so we don't have to use "ref" in all the patterns below.
    let args: Vec<&Json> = parts[1..].iter().collect();
    match (parts[0].as_string().unwrap(), &args[..]) {

        ("StartTag", [name, attrs, ..]) => XTagToken(XTag {
            kind: StartXTag,
            name: Atom::from_slice(name.as_string().unwrap()),
            attrs: attrs.as_object().unwrap().iter().map(|(k,v)| {
                Attribute {
                    name: QualName::new(ns!(""), Atom::from_slice(&k)),
                    value: v.as_string().unwrap().to_tendril()
                }
            }).collect(),
        }),

        ("EndTag", [name]) => XTagToken(XTag {
            kind: EndXTag,
            name: Atom::from_slice(name.as_string().unwrap()),
            attrs: vec!(),
        }),

        ("ShortTag", [name]) => XTagToken(XTag {
            kind: ShortXTag,
            name: Atom::from_slice(name.as_string().unwrap()),
            attrs: vec!(),
        }),

        ("EmptyTag", [name, attrs, ..]) => XTagToken(XTag {
            kind: EmptyXTag,
            name: Atom::from_slice(name.as_string().unwrap()),
            attrs: attrs.as_object().unwrap().iter().map(|(k,v)| {
                Attribute {
                    name: QualName::new(ns!(""), Atom::from_slice(&k)),
                    value: v.as_string().unwrap().to_tendril()
                }
            }).collect(),
        }),

        ("Comment", [txt]) => CommentXToken(txt.as_string().unwrap().to_tendril()),

        ("Character", [txt]) => CharacterXTokens(txt.as_string().unwrap().to_tendril()),

        ("PI", [target, data]) => PIToken(XPi {
            target: target.as_string().unwrap().to_tendril(), 
            data: data.as_string().unwrap().to_tendril(),
        }),

        // We don't need to produce NullCharacterToken because
        // the TokenLogger will convert them to CharacterTokens.

        _ => panic!("don't understand token {:?}", parts),
    }
}


// Parse the "output" field of the test case into a vector of tokens.
fn json_to_xtokens(js: &Json, exact_errors: bool) -> Vec<XToken> {
    // Use a TokenLogger so that we combine character tokens separated
    // by an ignored error.
    let mut sink = XTokenLogger::new(exact_errors);
    for tok in js.as_array().unwrap().iter() {
        match *tok {
            Json::String(ref s)
                if &s[..] == "ParseError" => sink.process_token(XParseError(Borrowed(""))),
            _ => sink.process_token(json_to_xtoken(tok)),
        }
    }
    sink.get_tokens()
}


fn mk_xml_test(desc: String, input: String, expect: Json, opts: XmlTokenizerOpts)
        -> TestDescAndFn {
    TestDescAndFn {
        desc: TestDesc {
            name: DynTestName(desc),
            ignore: false,
            should_panic: ShouldPanic::No,
        },
        testfn: DynTestFn(Box::new(move || {
            // Split up the input at different points to test incremental tokenization.
            let insplits = splits(&input, 3);
            for input in insplits.into_iter() {
                // Clone 'input' so we have it for the failure message.
                // Also clone opts.  If we don't, we get the wrong
                // result but the compiler doesn't catch it!
                // Possibly mozilla/rust#12223.
                let output = tokenize_xml(input.clone(), opts.clone());
                let expect = json_to_xtokens(&expect, opts.exact_errors);
                if output != expect {
                    panic!("\ninput: {:?}\ngot: {:?}\nexpected: {:?}",
                        input, output, expect);
                }
            }
        })),
    }
}

fn mk_xml_tests(tests: &mut Vec<TestDescAndFn>, filename: &str, js: &Json) {
    let input = js.find("input").unwrap().as_string().unwrap();
    let expect = js.find("output").unwrap().clone();
    let desc = format!("tok: {}: {}",
        filename, js.find("description").unwrap().as_string().unwrap());

    // Some tests want to start in a state other than Data.
    let state_overrides = vec!(None);


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

            tests.push(mk_xml_test(newdesc, String::from(input), expect.clone(), XmlTokenizerOpts {
                exact_errors: exact_errors,
                initial_state: state,

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
    foreach_xml5lib_test(src_dir, "tokenizer",
                         OsStr::new("test"), |path, mut file| {
        let js = Json::from_reader(&mut file).ok().expect("json parse error");

        match js["tests"] {
            Json::Array(ref lst) => {
                for test in lst.iter() {
                    mk_xml_tests(&mut tests, path.file_name().unwrap().to_str().unwrap(), test);
                }
            }

            _ => (),
        }

    });

    tests
}


#[start]
fn start(argc: isize, argv: *const *const u8) -> isize {
    unsafe {
        rt::args::init(argc, argv);
    }
    let args: Vec<_> = env::args().collect();
    test::test_main(&args, tests(Path::new(env!("CARGO_MANIFEST_DIR"))));
    0
}
