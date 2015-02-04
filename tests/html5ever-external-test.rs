// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_name="html5ever-external-test"]
#![crate_type="bin"]

#![feature(plugin)]
#![feature(rustc_private, core, io, os, path, std_misc, test)]

extern crate test;
extern crate serialize;
extern crate string_cache;
#[plugin]
#[macro_use]
extern crate string_cache_macros;

extern crate html5ever;

use std::old_io as io;
use std::os;
use std::str::FromStr;
use std::collections::HashSet;
use test::test_main;

mod tokenizer;
mod tree_builder;
mod util;

// Needed to make `cargo test` run warning-free.
#[allow(dead_code)]
fn main() {
    let src_dir: Path = FromStr::from_str(
        os::getenv("HTML5EVER_SRC_DIR").expect("HTML5EVER_SRC_DIR not set").as_slice()
    ).ok().expect("HTML5EVER_SRC_DIR invalid");

    let mut ignores = HashSet::new();
    {
        let f = io::File::open(&src_dir.join("data/test/ignore")).unwrap();
        let mut r = io::BufferedReader::new(f);
        for ln in r.lines() {
            ignores.insert(ln.unwrap().as_slice().trim_right().to_string());
        }
    }

    let mut tests = vec!();

    if os::getenv("HTML5EVER_NO_TOK_TEST").is_none() {
        tests.extend(tokenizer::tests(src_dir.clone()));
    }

    if os::getenv("HTML5EVER_NO_TB_TEST").is_none() {
        tests.extend(tree_builder::tests(src_dir, &ignores));
    }

    let args: Vec<String> = os::args().into_iter().collect();
    test_main(args.as_slice(), tests);
}
