// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use util::foreach_html5lib_test;

use std::io;
use std::mem::replace;
use std::default::Default;
use std::path::Path;
use std::collections::{HashSet, HashMap};
use std::vec::MoveItems;
use test::{TestDesc, TestDescAndFn, DynTestName, DynTestFn};

use html5ever::sink::common::{Document, Doctype, Text, Comment, Element};
use html5ever::sink::rcdom::{RcDom, Handle};
use html5ever::{parse, one_input};

fn parse_tests<It: Iterator<String>>(mut lines: It) -> Vec<HashMap<String, String>> {
    let mut tests = vec!();
    let mut test = HashMap::new();
    let mut key = None;
    let mut val = String::new();

    macro_rules! finish_val ( () => (
        match key.take() {
            None => (),
            Some(key) => assert!(test.insert(key, replace(&mut val, String::new())).is_none()),
        }
    ))

    macro_rules! finish_test ( () => (
        if !test.is_empty() {
            tests.push(replace(&mut test, HashMap::new()));
        }
    ))

    loop {
        match lines.next() {
            None => break,
            Some(line) => {
                if line.as_slice().starts_with("#") {
                    finish_val!();
                    if line.as_slice() == "#data\n" {
                        finish_test!();
                    }
                    key = Some(line.as_slice().slice_from(1)
                        .trim_right_chars('\n').to_string());
                } else {
                    val.push_str(line.as_slice());
                }
            }
        }
    }

    finish_val!();
    finish_test!();
    tests
}

fn serialize(buf: &mut String, indent: uint, handle: Handle) {
    buf.push_str("|");
    buf.grow(indent, ' ');

    let node = handle.borrow();
    match node.node {
        Document => panic!("should not reach Document"),

        Doctype(ref name, ref public, ref system) => {
            buf.push_str("<!DOCTYPE ");
            buf.push_str(name.as_slice());
            if !public.is_empty() || !system.is_empty() {
                buf.push_str(format!(" \"{}\" \"{}\"", public, system).as_slice());
            }
            buf.push_str(">\n");
        }

        Text(ref text) => {
            buf.push_str("\"");
            buf.push_str(text.as_slice());
            buf.push_str("\"\n");
        }

        Comment(ref text) => {
            buf.push_str("<!-- ");
            buf.push_str(text.as_slice());
            buf.push_str(" -->\n");
        }

        Element(ref name, ref attrs) => {
            assert!(name.ns == ns!(HTML));
            buf.push_str("<");
            buf.push_str(name.local.as_slice());
            buf.push_str(">\n");

            let mut attrs = attrs.clone();
            attrs.sort_by(|x, y| x.name.local.cmp(&y.name.local));
            // FIXME: sort by UTF-16 code unit

            for attr in attrs.into_iter() {
                assert!(attr.name.ns == ns!(""));
                buf.push_str("|");
                buf.grow(indent+2, ' ');
                buf.push_str(format!("{}=\"{}\"\n",
                    attr.name.local.as_slice(), attr.value).as_slice());
            }
        }
    }

    for child in node.children.iter() {
        serialize(buf, indent+2, child.clone());
    }
}

// Ignore tests containing these strings; we don't support these features yet.
static IGNORE_SUBSTRS: &'static [&'static str]
    = &["<math", "<svg", "<template"];

fn make_test(
        tests: &mut Vec<TestDescAndFn>,
        ignores: &HashSet<String>,
        path_str: &str,
        idx: uint,
        fields: HashMap<String, String>) {

    let get_field = |key| {
        let field = fields.find_equiv(key).expect("missing field");
        field.as_slice().trim_right_chars('\n').to_string()
    };

    if fields.find_equiv("document-fragment").is_some() {
        // FIXME
        return;
    }

    let data = get_field("data");
    let expected = get_field("document");
    let name = format!("tb: {}-{}", path_str, idx);
    let ignore = ignores.contains(&name)
        || IGNORE_SUBSTRS.iter().any(|&ig| data.as_slice().contains(ig));

    tests.push(TestDescAndFn {
        desc: TestDesc {
            name: DynTestName(name),
            ignore: ignore,
            should_fail: false,
        },
        testfn: DynTestFn(proc() {
            let dom: RcDom = parse(one_input(data.clone()), Default::default());

            let mut result = String::new();
            for child in dom.document.borrow().children.iter() {
                serialize(&mut result, 1, child.clone());
            }
            let len = result.len();
            result.truncate(len - 1);  // drop the trailing newline

            if result != expected {
                panic!("\ninput: {}\ngot:\n{}\nexpected:\n{}\n",
                    data, result, expected);
            }
        }),
    });
}

pub fn tests(src_dir: Path, ignores: &HashSet<String>) -> MoveItems<TestDescAndFn> {
    let mut tests = vec!();

    foreach_html5lib_test(src_dir, "tree-construction", ".dat", |path_str, file| {
        let mut buf = io::BufferedReader::new(file);
        let lines = buf.lines()
            .map(|res| res.ok().expect("couldn't read"));
        let data = parse_tests(lines);

        for (i, test) in data.into_iter().enumerate() {
            make_test(&mut tests, ignores, path_str, i, test);
        }
    });

    tests.into_iter()
}
