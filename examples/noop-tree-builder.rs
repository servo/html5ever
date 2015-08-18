// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![cfg_attr(feature = "unstable", feature(plugin))]
#![cfg_attr(feature = "unstable", plugin(string_cache_plugin))]

#[macro_use]
extern crate string_cache;
extern crate tendril;
extern crate html5ever;

use std::io;
use std::default::Default;
use std::collections::HashMap;
use std::borrow::Cow;
use string_cache::QualName;

use tendril::{StrTendril, ByteTendril, ReadExt};

use html5ever::{parse_to, one_input};
use html5ever::tokenizer::Attribute;
use html5ever::tree_builder::{TreeSink, QuirksMode, NodeOrText};

struct Sink {
    next_id: usize,
    names: HashMap<usize, QualName>,
}

impl Sink {
    fn get_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 2;
        id
    }
}

impl TreeSink for Sink {
    type Handle = usize;

    fn get_document(&mut self) -> usize {
        0
    }

    fn get_template_contents(&self, target: usize) -> usize {
        if let Some(&qualname!(HTML, template)) = self.names.get(&target) {
            target + 1
        } else {
            panic!("not a template element")
        }
    }

    fn same_node(&self, x: usize, y: usize) -> bool {
        x == y
    }

    fn elem_name(&self, target: usize) -> QualName {
        self.names.get(&target).expect("not an element").clone()
    }

    fn create_element(&mut self, name: QualName, _attrs: Vec<Attribute>) -> usize {
        let id = self.get_id();
        self.names.insert(id, name);
        id
    }

    fn create_comment(&mut self, _text: StrTendril) -> usize {
        self.get_id()
    }

    fn append_before_sibling(&mut self,
            _sibling: usize,
            _new_node: NodeOrText<usize>) -> Result<(), NodeOrText<usize>> {
        // `sibling` will have a parent unless a script moved it, and we're
        // not running scripts.  Therefore we can aways return `Ok(())`.
        Ok(())
    }

    fn parse_error(&mut self, _msg: Cow<'static, str>) { }
    fn set_quirks_mode(&mut self, _mode: QuirksMode) { }
    fn append(&mut self, _parent: usize, _child: NodeOrText<usize>) { }

    fn append_doctype_to_document(&mut self, _: StrTendril, _: StrTendril, _: StrTendril) { }
    fn add_attrs_if_missing(&mut self, target: usize, _attrs: Vec<Attribute>) {
        assert!(self.names.contains_key(&target), "not an element");
    }
    fn remove_from_parent(&mut self, _target: usize) { }
    fn reparent_children(&mut self, _node: usize, _new_parent: usize) { }
    fn mark_script_already_started(&mut self, _node: usize) { }
}

fn main() {
    let sink = Sink {
        next_id: 1,
        names: HashMap::new(),
    };

    let mut input = ByteTendril::new();
    io::stdin().read_to_tendril(&mut input).unwrap();
    let input = input.try_reinterpret().unwrap();
    parse_to(sink, one_input(input), Default::default());
}
