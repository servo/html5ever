/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate html5;

use std::io;
use std::default::Default;
use std::strbuf::StrBuf;

use html5::{Namespace, Atom};
use html5::tokenizer::{Tokenizer, Attribute};
use html5::tree_builder::{TreeBuilder, TreeSink, QuirksMode};

struct Sink {
    next_id: uint,
}

impl TreeSink<uint> for Sink {
    fn parse_error(&mut self, msg: ~str) {
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

    fn append_comment(&mut self, parent: uint, text: StrBuf) {
        println!("Append comment to {:u}: {:s}", parent, text);
    }

    fn append_element(&mut self, parent: uint, child: uint) {
        println!("Append element {:u} to {:u}", child, parent);
    }

    fn append_doctype_to_document(&mut self, name: StrBuf, public_id: StrBuf, system_id: StrBuf) {
        println!("Append doctype: {:s} {:s} {:s}", name, public_id, system_id);
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
