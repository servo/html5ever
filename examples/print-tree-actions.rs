// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unstable)]

extern crate string_cache;

extern crate html5ever;

use std::io;
use std::default::Default;
use std::string::String;
use std::collections::HashMap;
use std::string::CowString;
use string_cache::QualName;

use html5ever::{parse_to, one_input};
use html5ever::tokenizer::Attribute;
use html5ever::tree_builder::{TreeSink, QuirksMode, NodeOrText, AppendNode, AppendText};

struct Sink {
    next_id: uint,
    names: HashMap<uint, QualName>,
}

impl Sink {
    fn get_id(&mut self) -> uint {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

impl TreeSink for Sink {
    type Handle = uint;

    fn parse_error(&mut self, msg: CowString<'static>) {
        println!("Parse error: {}", msg);
    }

    fn get_document(&mut self) -> uint {
        0
    }

    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        println!("Set quirks mode to {:?}", mode);
    }

    fn same_node(&self, x: uint, y: uint) -> bool {
        x == y
    }

    fn elem_name(&self, target: uint) -> QualName {
        self.names.get(&target).expect("not an element").clone()
    }

    fn create_element(&mut self, name: QualName, _attrs: Vec<Attribute>) -> uint {
        let id = self.get_id();
        println!("Created {:?} as {}", name, id);
        self.names.insert(id, name);
        id
    }

    fn create_comment(&mut self, text: String) -> uint {
        let id = self.get_id();
        println!("Created comment \"{}\" as {}", text.escape_default(), id);
        id
    }

    fn append(&mut self, parent: uint, child: NodeOrText<uint>) {
        match child {
            AppendNode(n)
                => println!("Append node {} to {}", n, parent),
            AppendText(t)
                => println!("Append text to {}: \"{}\"", parent, t.escape_default()),
        }
    }

    fn append_before_sibling(&mut self,
            sibling: uint,
            new_node: NodeOrText<uint>) -> Result<(), NodeOrText<uint>> {
        match new_node {
            AppendNode(n)
                => println!("Append node {} before {}", n, sibling),
            AppendText(t)
                => println!("Append text before {}: \"{}\"", sibling, t.escape_default()),
        }

        // `sibling` will have a parent unless a script moved it, and we're
        // not running scripts.  Therefore we can aways return `Ok(())`.
        Ok(())
    }

    fn append_doctype_to_document(&mut self, name: String, public_id: String, system_id: String) {
        println!("Append doctype: {} {} {}", name, public_id, system_id);
    }

    fn add_attrs_if_missing(&mut self, target: uint, attrs: Vec<Attribute>) {
        println!("Add missing attributes to {}:", target);
        for attr in attrs.into_iter() {
            println!("    {:?} = {}", attr.name, attr.value);
        }
    }

    fn remove_from_parent(&mut self, target: uint) {
        println!("Remove {} from parent", target);
    }

    fn mark_script_already_started(&mut self, node: uint) {
        println!("Mark script {} as already started", node);
    }
}

fn main() {
    let sink = Sink {
        next_id: 1,
        names: HashMap::new(),
    };

    let input = io::stdin().read_to_string().unwrap();
    parse_to(sink, one_input(input), Default::default());
}
