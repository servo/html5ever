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
use std::string::String;

use html5ever::sink::common::{Document, Doctype, Text, Comment, Element};
use html5ever::sink::rcdom::{RcDom, Handle};
use html5ever::{parse, one_input};

// This is not proper HTML serialization, of course.

fn walk(indent: uint, handle: Handle) {
    let node = handle.borrow();
    // FIXME: don't allocate
    print!("{:s}", String::from_char(indent, ' '));
    match node.node {
        Document
            => println!("#Document"),

        Doctype(ref name, ref public, ref system)
            => println!("<!DOCTYPE {:s} \"{:s}\" \"{:s}\">", *name, *public, *system),

        Text(ref text)
            => println!("#text: {:s}", text.escape_default()),

        Comment(ref text)
            => println!("<!-- {:s} -->", text.escape_default()),

        Element(ref name, ref attrs) => {
            print!("<{:s}", name.as_slice());
            for attr in attrs.iter() {
                print!(" {:s}=\"{:s}\"", attr.name.name.as_slice(), attr.value);
            }
            println!(">");
        }
    }

    for child in node.children.iter() {
        walk(indent+4, child.clone());
    }
}

fn main() {
    let input = io::stdin().read_to_string().unwrap();
    let dom: RcDom = parse(one_input(input), Default::default());
    walk(0, dom.document);

    if !dom.errors.is_empty() {
        println!("\nParse errors:");
        for err in dom.errors.move_iter() {
            println!("    {}", err);
        }
    }
}
