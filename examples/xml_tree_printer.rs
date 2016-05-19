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

use std::io::{self, Read};
use std::default::Default;
use std::string::String;
use std::iter;

use xml5ever::tendril::{ByteTendril, ReadExt};
use xml5ever::driver::{parse};
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
    // We need to allocate an input tendril for xml5ever
    let mut input = ByteTendril::new();
    // Using ReadExt.read_to_tendril functions we can read stdin
    io::stdin().read_to_tendril(&mut input).unwrap();
    let input = input.try_reinterpret().unwrap();

    // To parse XML into a tree form, we need a TreeSink
    // luckily xml5ever comes with a static RC backed tree represetation.
    let dom: RcDom = parse(iter::once(input), Default::default());

    // Execute our visualizer on RcDom
    walk("", dom.document);
}
