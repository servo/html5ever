/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[crate_id="html5-external-test"];
#[crate_type="bin"];

extern crate test;
extern crate serialize;
extern crate collections;

extern crate html5;

use std::os;
use test::test_main;

mod tokenizer;

fn main() {
    let mut tests = Vec::new();

    tests.push_all_move(tokenizer::tests());
    // more to follow

    let args = os::args();
    test_main(args.as_slice(), tests);
}
