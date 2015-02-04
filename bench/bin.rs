// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_name="html5ever-external-bench"]
#![crate_type="bin"]

#![feature(box_syntax)]
#![feature(core, io, os, path, test)]

extern crate test;

extern crate html5ever;

use std::os;
use test::test_main;

mod tokenizer;

fn main() {
    let mut tests = vec!();

    tests.extend(tokenizer::tests());
    // more to follow

    test_main(os::args().as_slice(), tests);
}
