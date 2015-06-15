// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Parse and re-serialize a HTML5 document.
//!
//! This is meant to produce the exact same output (ignoring stderr) as
//!
//!   java -classpath htmlparser-1.4.jar nu.validator.htmlparser.tools.HTML2HTML
//!
//! where htmlparser-1.4.jar comes from http://about.validator.nu/htmlparser/

extern crate html5ever;
extern crate html5ever_dom_sink;

use std::io::{self, Read, Write};
use std::default::Default;

use html5ever::driver::ParseOpts;
use html5ever_dom_sink::rcdom::RcDom;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{parse, one_input, serialize};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let dom: RcDom = parse(one_input(input), ParseOpts {
        tree_builder: TreeBuilderOpts {
            drop_doctype: true,
            ..Default::default()
        },
        ..Default::default()
    });

    // The validator.nu HTML2HTML always prints a doctype at the very beginning.
    io::stdout().write_all(b"<!DOCTYPE html>\n")
        .ok().expect("writing DOCTYPE failed");
    serialize(&mut io::stdout(), &dom.document, Default::default())
        .ok().expect("serialization failed");
}
