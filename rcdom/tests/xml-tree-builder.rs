// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use markup5ever::{namespace_url, ns};
use markup5ever_rcdom::*;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::io::BufRead;
use std::path::Path;
use std::{env, fs, io, iter, mem};
use util::find_tests::foreach_xml5lib_test;
use util::runner::{run_all, Test};
use xml5ever::driver::parse_document;
use xml5ever::tendril::TendrilSink;

mod util {
    pub mod find_tests;
    pub mod runner;
}

fn parse_tests<It: Iterator<Item = String>>(mut lines: It) -> Vec<HashMap<String, String>> {
    let mut tests = vec![];
    let mut test = HashMap::new();
    let mut key: Option<String> = None;
    let mut val = String::new();

    macro_rules! finish_val ( () => (
        match key.take() {
            None => (),
            Some(key) => {
                assert!(test.insert(key, mem::take(&mut val)).is_none());
            }
        }
    ));

    macro_rules! finish_test ( () => (
        if !test.is_empty() {
            tests.push(mem::take(&mut test));
        }
    ));

    loop {
        match lines.next() {
            None => break,
            Some(line) => {
                if let Some(rest) = line.strip_prefix('#') {
                    finish_val!();
                    if line == "#data" {
                        finish_test!();
                    }
                    key = Some(rest.to_string());
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
    buf.push('|');
    buf.extend(iter::repeat(" ").take(indent));

    let node = handle;
    match &node.data {
        NodeData::Document => panic!("should not reach Document"),

        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => {
            buf.push_str("<!DOCTYPE ");
            buf.push_str(name);
            if !public_id.is_empty() || !system_id.is_empty() {
                buf.push_str(&format!(" \"{}\" \"{}\"", public_id, system_id));
            }
            buf.push_str(">\n");
        },

        NodeData::Text { contents } => {
            buf.push('"');
            buf.push_str(&contents.borrow());
            buf.push_str("\"\n");
        },

        NodeData::ProcessingInstruction { target, contents } => {
            buf.push_str("<?");
            buf.push_str(target);
            buf.push(' ');
            buf.push_str(contents);
            buf.push_str("?>\n");
        },

        NodeData::Comment { contents } => {
            buf.push_str("<!-- ");
            buf.push_str(contents);
            buf.push_str(" -->\n");
        },

        NodeData::Element { name, attrs, .. } => {
            buf.push('<');

            if name.ns != ns!() {
                buf.push('{');
                buf.push_str(&name.ns);
                buf.push('}');
            };

            if let Some(prefix) = &name.prefix {
                buf.push_str(prefix);
                buf.push(':');
            }

            buf.push_str(&name.local);
            buf.push_str(">\n");

            let mut attrs = attrs.borrow().clone();
            attrs.sort_by(|x, y| x.name.local.cmp(&y.name.local));
            // FIXME: sort by UTF-16 code unit

            for attr in attrs.into_iter() {
                buf.push('|');
                buf.extend(iter::repeat(" ").take(indent + 2));

                if !attr.name.ns.is_empty() {
                    buf.push('{');
                    buf.push_str(&attr.name.ns);
                    buf.push('}');
                }

                if let Some(attr_prefix) = attr.name.prefix {
                    buf.push_str(&attr_prefix);
                    buf.push(':');
                }

                buf.push_str(&format!("{}=\"{}\"\n", attr.name.local, attr.value));
            }
        },
    }

    for child in node.children.borrow().iter() {
        serialize(buf, indent + 2, child.clone());
    }
}

// Ignore tests containing these strings; we don't support these features yet.
static IGNORE_SUBSTRS: &[&str] = &["<template"];

fn make_xml_test(
    tests: &mut Vec<Test>,
    ignores: &HashSet<String>,
    filename: &str,
    idx: usize,
    fields: HashMap<String, String>,
) {
    let get_field = |key| {
        let field = fields.get(key).expect("missing field");
        field.trim_end_matches('\n').to_string()
    };

    let data = get_field("data");
    let expected = get_field("document");
    let name = format!("tb: {}-{}", filename, idx);
    let skip = ignores.contains(&name) || IGNORE_SUBSTRS.iter().any(|&ig| data.contains(ig));

    tests.push(Test {
        name,
        skip,
        test: Box::new(move || {
            let mut result = String::new();

            let dom = parse_document(RcDom::default(), Default::default()).one(data.clone());
            for child in dom.document.children.borrow().iter() {
                serialize(&mut result, 1, child.clone());
            }

            let len = result.len();
            result.truncate(len - 1); // drop the trailing newline

            if result != expected {
                panic!(
                    "\ninput: {}\ngot:\n{}\nexpected:\n{}\n",
                    data, result, expected
                );
            }
        }),
    });
}

fn tests(src_dir: &Path, ignores: &HashSet<String>) -> Vec<Test> {
    let mut tests = vec![];

    foreach_xml5lib_test(
        src_dir,
        "tree-construction",
        OsStr::new("dat"),
        |path, file| {
            let buf = io::BufReader::new(file);
            let lines = buf.lines().map(|res| res.expect("couldn't read"));
            let data = parse_tests(lines);

            for (i, test) in data.into_iter().enumerate() {
                make_xml_test(
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
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut ignores = HashSet::new();
    if let Ok(f) = fs::File::open(src_dir.join("data/test/ignore")) {
        let r = io::BufReader::new(f);
        for ln in r.lines() {
            ignores.insert(ln.unwrap().trim_end().to_string());
        }
    }

    run_all(tests(src_dir, &ignores));
}
