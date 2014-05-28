// Copyright 2014 The HTML5 for Rust Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate html5;

use std::io;
use std::default::Default;
use std::string::String;

use html5::{Namespace, Atom};
use html5::tokenizer::{Tokenizer, Attribute};
use html5::tree_builder::{TreeBuilder, TreeSink, QuirksMode};

struct Sink {
    next_id: uint,
}

impl TreeSink<uint> for Sink {
    fn parse_error(&mut self, msg: String) {
        println!("Parse error: {:s}", msg);
    }

    fn get_document(&mut self) -> uint {
        0
    }

    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        println!("Set quirks mode to {:?}", mode);
    }

    fn create_element(&mut self, ns: Namespace, name: Atom, _attrs: Vec<Attribute>) -> uint {
        let id = self.next_id;
        self.next_id += 1;
        println!("Created {:?}:{:s} as {:u}", ns, name, id);
        id
    }

    fn append_text(&mut self, parent: uint, text: String) {
        println!("Append text to {:u}: {:s}", parent, text);
    }

    fn append_comment(&mut self, parent: uint, text: String) {
        println!("Append comment to {:u}: {:s}", parent, text);
    }

    fn append_element(&mut self, parent: uint, child: uint) {
        println!("Append element {:u} to {:u}", child, parent);
    }

    fn append_doctype_to_document(&mut self, name: String, public_id: String, system_id: String) {
        println!("Append doctype: {:s} {:s} {:s}", name, public_id, system_id);
    }

    fn mark_script_already_started(&mut self, node: uint) {
        println!("Mark script {:u} as already started", node);
    }
}


fn main() {
    let mut sink = Sink {
        next_id: 1,
    };

    let mut tb  = TreeBuilder::new(&mut sink, Default::default());
    let mut tok = Tokenizer::new(&mut tb, Default::default());

    tok.feed(io::stdin().read_to_str().unwrap().into_strbuf());
    tok.end();
}
