// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate rustc_test as test;
#[macro_use]
extern crate html5ever;

mod foreach_html5lib_test;
use foreach_html5lib_test::foreach_html5lib_test;

use std::collections::{HashMap, HashSet};
use std::default::Default;
use std::ffi::OsStr;
use std::io::BufRead;
use std::iter::repeat;
use std::mem::replace;
use std::path::Path;
use std::{env, fs, io};
use test::{DynTestName, TestDesc, TestDescAndFn, TestFn};

use html5ever::rcdom::{Handle, NodeData, RcDom};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{parse_document, parse_fragment, ParseOpts};
use html5ever::{LocalName, QualName};

fn parse_tests<It: Iterator<Item = String>>(mut lines: It) -> Vec<HashMap<String, String>> {
    let mut tests = vec![];
    let mut test = HashMap::new();
    let mut key: Option<String> = None;
    let mut val = String::new();

    macro_rules! finish_val ( () => (
        match key.take() {
            None => (),
            Some(key) => {
                assert!(test.insert(key, replace(&mut val, String::new())).is_none());
            }
        }
    ));

    macro_rules! finish_test ( () => (
        if !test.is_empty() {
            tests.push(replace(&mut test, HashMap::new()));
        }
    ));

    loop {
        match lines.next() {
            None => break,
            Some(line) => {
                if line.starts_with("#") {
                    finish_val!();
                    if line == "#data" {
                        finish_test!();
                    }
                    key = Some(line[1..].to_string());
                } else {
                    val.push_str(&line);
                    val.push('\n');
                }
            },
        }
    }

    finish_val!();
    finish_test!();
    tests
}

fn serialize(buf: &mut String, indent: usize, handle: Handle) {
    buf.push_str("|");
    buf.push_str(&repeat(" ").take(indent).collect::<String>());

    let node = handle;
    match node.data {
        NodeData::Document => panic!("should not reach Document"),

        NodeData::Doctype {
            ref name,
            ref public_id,
            ref system_id,
        } => {
            buf.push_str("<!DOCTYPE ");
            buf.push_str(&name);
            if !public_id.is_empty() || !system_id.is_empty() {
                buf.push_str(&format!(" \"{}\" \"{}\"", public_id, system_id));
            }
            buf.push_str(">\n");
        },

        NodeData::Text { ref contents } => {
            buf.push_str("\"");
            buf.push_str(&contents.borrow());
            buf.push_str("\"\n");
        },

        NodeData::Comment { ref contents } => {
            buf.push_str("<!-- ");
            buf.push_str(&contents);
            buf.push_str(" -->\n");
        },

        NodeData::Element {
            ref name,
            ref attrs,
            ..
        } => {
            buf.push_str("<");
            match name.ns {
                ns!(svg) => buf.push_str("svg "),
                ns!(mathml) => buf.push_str("math "),
                _ => (),
            }
            buf.push_str(&*name.local);
            buf.push_str(">\n");

            let mut attrs = attrs.borrow().clone();
            attrs.sort_by(|x, y| x.name.local.cmp(&y.name.local));
            // FIXME: sort by UTF-16 code unit

            for attr in attrs.into_iter() {
                buf.push_str("|");
                buf.push_str(&repeat(" ").take(indent + 2).collect::<String>());
                match attr.name.ns {
                    ns!(xlink) => buf.push_str("xlink "),
                    ns!(xml) => buf.push_str("xml "),
                    ns!(xmlns) => buf.push_str("xmlns "),
                    _ => (),
                }
                buf.push_str(&format!("{}=\"{}\"\n", attr.name.local, attr.value));
            }
        },

        NodeData::ProcessingInstruction { .. } => unreachable!(),
    }

    for child in node.children.borrow().iter() {
        serialize(buf, indent + 2, child.clone());
    }

    if let NodeData::Element {
        template_contents: Some(ref content),
        ..
    } = node.data
    {
        buf.push_str("|");
        buf.push_str(&repeat(" ").take(indent + 2).collect::<String>());
        buf.push_str("content\n");
        for child in content.children.borrow().iter() {
            serialize(buf, indent + 4, child.clone());
        }
    }
}

fn make_test(
    tests: &mut Vec<TestDescAndFn>,
    ignores: &HashSet<String>,
    filename: &str,
    idx: usize,
    fields: HashMap<String, String>,
) {
    let scripting_flags = &[false, true];
    let scripting_flags = if fields.contains_key("script-off") {
        &scripting_flags[0..1]
    } else if fields.contains_key("script-on") {
        &scripting_flags[1..2]
    } else {
        &scripting_flags[0..2]
    };
    let name = format!("tb: {}-{}", filename, idx);
    for scripting_enabled in scripting_flags {
        let test = make_test_desc_with_scripting_flag(ignores, &name, &fields, *scripting_enabled);
        tests.push(test);
    }
}

fn make_test_desc_with_scripting_flag(
    ignores: &HashSet<String>,
    name: &str,
    fields: &HashMap<String, String>,
    scripting_enabled: bool,
) -> TestDescAndFn {
    let get_field = |key| {
        let field = fields.get(key).expect("missing field");
        field.trim_right_matches('\n').to_string()
    };

    let mut data = fields.get("data").expect("missing data").to_string();
    data.pop();
    let expected = get_field("document");
    let context = fields
        .get("document-fragment")
        .map(|field| context_name(field.trim_right_matches('\n')));
    let ignore = ignores.contains(name);
    let mut name = name.to_owned();
    if scripting_enabled {
        name.push_str(" (scripting enabled)");
    } else {
        name.push_str(" (scripting disabled)");
    };
    let mut opts: ParseOpts = Default::default();
    opts.tree_builder.scripting_enabled = scripting_enabled;

    TestDescAndFn {
        desc: TestDesc {
            ignore: ignore,
            ..TestDesc::new(DynTestName(name))
        },
        testfn: TestFn::dyn_test_fn(move || {
            // Do this here because Tendril isn't Send.
            let data = StrTendril::from_slice(&data);
            let mut result = String::new();
            match context {
                None => {
                    let dom = parse_document(RcDom::default(), opts).one(data.clone());
                    for child in dom.document.children.borrow().iter() {
                        serialize(&mut result, 1, child.clone());
                    }
                },
                Some(ref context) => {
                    let dom = parse_fragment(RcDom::default(), opts, context.clone(), vec![])
                        .one(data.clone());
                    // fragment case: serialize children of the html element
                    // rather than children of the document
                    let doc = &dom.document;
                    let root = &doc.children.borrow()[0];
                    for child in root.children.borrow().iter() {
                        serialize(&mut result, 1, child.clone());
                    }
                },
            };
            let len = result.len();
            result.truncate(len - 1); // drop the trailing newline

            if result != expected {
                panic!(
                    "\ninput: {}\ngot:\n{}\nexpected:\n{}\n",
                    data, result, expected
                );
            }
        }),
    }
}

fn context_name(context: &str) -> QualName {
    if context.starts_with("svg ") {
        QualName::new(None, ns!(svg), LocalName::from(&context[4..]))
    } else if context.starts_with("math ") {
        QualName::new(None, ns!(mathml), LocalName::from(&context[5..]))
    } else {
        QualName::new(None, ns!(html), LocalName::from(context))
    }
}

fn tests(src_dir: &Path, ignores: &HashSet<String>) -> Vec<TestDescAndFn> {
    let mut tests = vec![];

    foreach_html5lib_test(
        src_dir,
        "tree-construction",
        OsStr::new("dat"),
        |path, file| {
            let buf = io::BufReader::new(file);
            let lines = buf.lines().map(|res| res.ok().expect("couldn't read"));
            let data = parse_tests(lines);

            for (i, test) in data.into_iter().enumerate() {
                make_test(
                    &mut tests,
                    ignores,
                    path.file_name().unwrap().to_str().unwrap(),
                    i,
                    test,
                );
            }
        },
    );

    tests
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut ignores = HashSet::new();
    {
        let f = fs::File::open(&src_dir.join("data/test/ignore")).unwrap();
        let r = io::BufReader::new(f);
        for ln in r.lines() {
            ignores.insert(ln.unwrap().trim_right().to_string());
        }
    }

    test::test_main(&args, tests(src_dir, &ignores));
}
