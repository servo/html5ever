// Copyright 2014 The HTML5 for Rust Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_id="html5-external-test"]
#![crate_type="bin"]

extern crate test;
extern crate serialize;
extern crate debug;

extern crate html5;

use std::os;
use std::from_str::FromStr;
use test::test_main;

mod tokenizer;

fn main() {
    let src_dir: Path = FromStr::from_str(
        os::getenv("HTML5_SRC_DIR").expect("HTML5_SRC_DIR not set").as_slice()
    ).expect("HTML5_SRC_DIR invalid");

    let mut tests = vec!();

    tests.push_all_move(tokenizer::tests(src_dir));
    // more to follow

    let args: Vec<String> = os::args().move_iter().map(|x| x.into_string()).collect();
    test_main(args.as_slice(), tests);
}
