#!/usr/bin/env run-cargo-script
//! This is a regular crate doc comment, but it also contains a partial
//! Cargo manifest.  Note the use of a *fenced* code block, and the
//! `cargo` "language".
//!
//! ```cargo
//! [dependencies]
//! xml5ever = "0.2.0"
//! tendril = "0.1.3"
//! ```
extern crate xml5ever;

use std::default::Default;

use xml5ever::tendril::TendrilSink;
use xml5ever::driver::{parse_document, BytesOpts};
use xml5ever::tree_builder::{TreeSink};
use xml5ever::rcdom::{RcDom, Text};

fn main() {
    // To parse a string into a tree of nodes, we need to invoke
    // `parse_document` and supply it with a TreeSink implementation (RcDom).
    //
    // Since this is a string, it's best to use `from_bytes` to create a
    // BytesParser for given string.
    let dom: RcDom = parse_document(RcDom::default(), Default::default())
        .from_bytes(BytesOpts::default())
        .one("<hello>XML</hello>".as_bytes());

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
