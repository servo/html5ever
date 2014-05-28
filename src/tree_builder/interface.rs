// Copyright 2014 The HTML5 for Rust Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use util::atom::Atom;
use util::namespace::Namespace;
use tokenizer::Attribute;

#[deriving(Eq, TotalEq, Clone, Hash, Show)]
pub enum QuirksMode {
    Quirks,
    LimitedQuirks,
    NoQuirks,
}

pub trait TreeSink<Handle> {
    fn parse_error(&mut self, msg: String);
    fn get_document(&mut self) -> Handle;
    fn set_quirks_mode(&mut self, mode: QuirksMode);

    fn create_element(&mut self, ns: Namespace, name: Atom, attrs: Vec<Attribute>) -> Handle;

    fn append_text(&mut self, parent: Handle, text: String);
    fn append_comment(&mut self, parent: Handle, text: String);
    fn append_element(&mut self, parent: Handle, child: Handle);
    fn append_doctype_to_document(&mut self, name: String, public_id: String, system_id: String);

    fn mark_script_already_started(&mut self, node: Handle);
}
