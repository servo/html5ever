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

use std::io::{self};
use std::default::Default;
use std::string::String;

use xml5ever::tendril::{TendrilSink};
use xml5ever::driver::{parse_document};
use xml5ever::rcdom::{Document, Text, Element, RcDom, Handle};

fn walk(prefix: &str, handle: Handle) {
    let node = handle.borrow();

    print!("{}", prefix);
    match node.node {
        Document
            => println!("#document"),

        Text(ref text)  => {
            println!("#text {}", escape_default(text))
        },

        Element(ref name, _) => {
            println!("{}", name.local);
        },

        _ => {},

    }

    let new_indent = {
        let mut temp = String::new();
        temp.push_str(prefix);
        temp.push_str("    ");
        temp
    };

    for child in node.children.iter()
        .filter(|child| match child.borrow().node {
            Text(_) | Element (_, _) => true,
            _ => false,
        }
    ) {
        walk(&new_indent, child.clone());
    }
}

pub fn escape_default(s: &str) -> String {
    s.chars().flat_map(|c| c.escape_default()).collect()
}

fn main() {
    let stdin = io::stdin();

    // To parse XML into a tree form, we need a TreeSink
    // luckily xml5ever comes with a static RC backed tree represetation.
    let dom: RcDom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut stdin.lock())
        .unwrap();;

    // Execute our visualizer on RcDom
    walk("", dom.document);
}
