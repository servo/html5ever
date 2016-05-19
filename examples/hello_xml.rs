#!/usr/bin/env run-cargo-script
//! This is a regular crate doc comment, but it also contains a partial
//! Cargo manifest.  Note the use of a *fenced* code block, and the
//! `cargo` "language".
//!
//! ```cargo
//! [dependencies]
//! xml5ever = "0.1.1"
//! tendril = "0.1.3"
//! ```
extern crate xml5ever;

use std::default::Default;
use std::iter;

use xml5ever::tendril::{SliceExt};
use xml5ever::driver::{parse};
use xml5ever::tree_builder::{TreeSink};
use xml5ever::rcdom::{RcDom, Text};

fn main() {
    // Using SliceExt.to_tendril functions we can read stdin
    let input = "<hello>XML</hello>".to_tendril();

    // To parse XML into a tree form, we need a TreeSink
    // luckily xml5ever comes with a static RC backed tree represetation.
    let dom: RcDom = parse(iter::once(input), Default::default());

    // Do some processing
    let doc = &dom.document;

    let hello_node = &doc.borrow().children[0];
    let hello_tag = &*dom.elem_name(hello_node).local;
    let text_node = &hello_node.borrow().children[0];

    let xml = {
        let mut xml = String::new();

        match &text_node.borrow().node {
            &Text(ref text) => {
                xml.push_str(&*text);
            },
            e => {println!("{:?}", e);},
        };

        xml
    };

    println!("{:?} {:?}!", hello_tag, xml);
}
