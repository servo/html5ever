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

#![feature(macro_rules)]

extern crate test;
extern crate serialize;
extern crate debug;
extern crate string_cache;

extern crate html5ever;

use std::os;
use std::from_str::FromStr;
use test::test_main;

mod tokenizer;
mod tree_builder;
mod util;

fn main() {
    let src_dir: Path = FromStr::from_str(
        os::getenv("HTML5EVER_SRC_DIR").expect("HTML5EVER_SRC_DIR not set").as_slice()
    ).expect("HTML5EVER_SRC_DIR invalid");

    let mut tests = vec!();

    if os::getenv("HTML5EVER_NO_TOK_TEST").is_none() {
        tests.push_all_move(tokenizer::tests(src_dir.clone()));
    }

    if os::getenv("HTML5EVER_NO_TB_TEST").is_none() {
        tests.push_all_move(tree_builder::tests(src_dir));
    }

    let args: Vec<String> = os::args().move_iter().collect();
    test_main(args.as_slice(), tests);
}
