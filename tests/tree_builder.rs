// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![cfg_attr(feature = "unstable", feature(start, rt, test, plugin))]
#![cfg_attr(feature = "unstable", plugin(string_cache_plugin))]

#[cfg(feature = "unstable")] extern crate test;
#[macro_use] extern crate string_cache;
extern crate tendril;

extern crate html5ever;

mod foreach_html5lib_test;
use foreach_html5lib_test::foreach_html5lib_test;

use std::{fs, io, env, rt};
use std::io::BufRead;
use std::ffi::OsStr;
use std::iter::repeat;
use std::mem::replace;
use std::default::Default;
use std::path::Path;
use std::collections::{HashSet, HashMap};
#[cfg(feature = "unstable")] use test::{TestDesc, TestDescAndFn, DynTestName, DynTestFn};
#[cfg(feature = "unstable")] use test::ShouldPanic::No;

use html5ever::{parse, parse_fragment, one_input};
use html5ever::rcdom::{Document, Doctype, Text, Comment, Element, RcDom, Handle};

use string_cache::Atom;
use tendril::StrTendril;

fn parse_tests<It: Iterator<Item=String>>(mut lines: It) -> Vec<HashMap<String, String>> {
    let mut tests = vec!();
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
            }
        }
    }

    finish_val!();
    finish_test!();
    tests
}

fn serialize(buf: &mut String, indent: usize, handle: Handle) {
    buf.push_str("|");
    buf.push_str(&repeat(" ").take(indent).collect::<String>());

    let node = handle.borrow();
    match node.node {
        Document => panic!("should not reach Document"),

        Doctype(ref name, ref public, ref system) => {
            buf.push_str("<!DOCTYPE ");
            buf.push_str(&name);
            if !public.is_empty() || !system.is_empty() {
                buf.push_str(&format!(" \"{}\" \"{}\"", public, system));
            }
            buf.push_str(">\n");
        }

        Text(ref text) => {
            buf.push_str("\"");
            buf.push_str(&text);
            buf.push_str("\"\n");
        }

        Comment(ref text) => {
            buf.push_str("<!-- ");
            buf.push_str(&text);
            buf.push_str(" -->\n");
        }

        Element(ref name, ref attrs) => {
            buf.push_str("<");
            match name.ns {
                ns!(SVG) => buf.push_str("svg "),
                ns!(MathML) => buf.push_str("math "),
                _ => (),
            }
            buf.push_str(&*name.local);
            buf.push_str(">\n");

            let mut attrs = attrs.clone();
            attrs.sort_by(|x, y| x.name.local.cmp(&y.name.local));
            // FIXME: sort by UTF-16 code unit

            for attr in attrs.into_iter() {
                buf.push_str("|");
                buf.push_str(&repeat(" ").take(indent+2).collect::<String>());
                match attr.name.ns {
                    ns!(XLink) => buf.push_str("xlink "),
                    ns!(XML) => buf.push_str("xml "),
                    ns!(XMLNS) => buf.push_str("xmlns "),
                    _ => (),
                }
                buf.push_str(&format!("{}=\"{}\"\n",
                    attr.name.local, attr.value));
            }
        }
    }

    for child in node.children.iter() {
        serialize(buf, indent+2, child.clone());
    }
}

// Ignore tests containing these strings; we don't support these features yet.
static IGNORE_SUBSTRS: &'static [&'static str]
    = &["<template"];

#[cfg(feature = "unstable")]
fn make_test(
        tests: &mut Vec<TestDescAndFn>,
        ignores: &HashSet<String>,
        filename: &str,
        idx: usize,
        fields: HashMap<String, String>) {

    let get_field = |key| {
        let field = fields.get(key).expect("missing field");
        field.trim_right_matches('\n').to_string()
    };

    let data = get_field("data");
    let expected = get_field("document");
    let context = fields.get("document-fragment")
                        .map(|field| Atom::from_slice(field.trim_right_matches('\n')));
    let name = format!("tb: {}-{}", filename, idx);
    let ignore = ignores.contains(&name)
        || IGNORE_SUBSTRS.iter().any(|&ig| data.contains(ig));

    tests.push(TestDescAndFn {
        desc: TestDesc {
            name: DynTestName(name),
            ignore: ignore,
            should_panic: No,
        },
        testfn: DynTestFn(Box::new(move || {
            // Do this here because Tendril isn't Send.
            let data = StrTendril::from_slice(&data);
            let mut result = String::new();
            match context {
                None => {
                    let dom: RcDom = parse(one_input(data.clone()), Default::default());
                    for child in dom.document.borrow().children.iter() {
                        serialize(&mut result, 1, child.clone());
                    }
                },
                Some(ref context) => {
                    let dom: RcDom = parse_fragment(one_input(data.clone()),
                                                    context.clone(),
                                                    Default::default());
                    // fragment case: serialize children of the html element
                    // rather than children of the document
                    let doc = dom.document.borrow();
                    let root = doc.children[0].borrow();
                    for child in root.children.iter() {
                        serialize(&mut result, 1, child.clone());
                    }
                },
            };
            let len = result.len();
            result.truncate(len - 1);  // drop the trailing newline

            if result != expected {
                panic!("\ninput: {}\ngot:\n{}\nexpected:\n{}\n",
                    data, result, expected);
            }
        })),
    });
}

#[cfg(feature = "unstable")]
fn tests(src_dir: &Path, ignores: &HashSet<String>) -> Vec<TestDescAndFn> {
    let mut tests = vec!();

    foreach_html5lib_test(src_dir, "tree-construction",
                          OsStr::new("dat"), |path, file| {
        let buf = io::BufReader::new(file);
        let lines = buf.lines()
            .map(|res| res.ok().expect("couldn't read"));
        let data = parse_tests(lines);

        for (i, test) in data.into_iter().enumerate() {
            make_test(&mut tests, ignores, path.file_name().unwrap().to_str().unwrap(),
                      i, test);
        }
    });

    tests
}

#[cfg(feature = "unstable")]
#[start]
fn start(argc: isize, argv: *const *const u8) -> isize {
    unsafe {
        rt::args::init(argc, argv);
    }
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
    0
}
