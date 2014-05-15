/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#![crate_id="html5-external-test"]
#![crate_type="bin"]

extern crate test;
extern crate serialize;
extern crate collections;

extern crate html5;

use std::os;
use std::from_str::FromStr;
use test::test_main;

mod tokenizer;

fn main() {
    let src_dir: Path = FromStr::from_str(
        os::getenv("HTML5_SRC_DIR").expect("HTML5_SRC_DIR not set")
    ).expect("HTML5_SRC_DIR invalid");

    let mut tests = vec!();

    tests.push_all_move(tokenizer::tests(src_dir));
    // more to follow

    let args = os::args();
    test_main(args.as_slice(), tests);
}
