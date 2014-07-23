// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate html5ever;

use std::io;
use std::default::Default;

use html5ever::sink::rcdom::RcDom;
use html5ever::{parse, one_input, serialize};

fn main() {
    let input = io::stdin().read_to_str().unwrap();
    let dom: RcDom = parse(one_input(input), Default::default());
    serialize(&mut io::stdout(), &dom.document, Default::default())
        .ok().expect("serialization failed");
}
