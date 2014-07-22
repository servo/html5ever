// Copyright 2014 The html5ever Project Developers. See the
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

/// A document's quirks mode.
#[deriving(PartialEq, Eq, Clone, Hash, Show)]
pub enum QuirksMode {
    Quirks,
    LimitedQuirks,
    NoQuirks,
}

/// Types which can process tree modifications from the tree builder.
///
/// `Handle` is a reference to a DOM node.  The tree builder requires
/// that a `Handle` implements `Clone` to get another reference to
/// the same node.
pub trait TreeSink<Handle> {
    /// Signal a parse error.
    fn parse_error(&mut self, msg: String);

    /// Get a handle to the `Document` node.
    fn get_document(&mut self) -> Handle;

    /// Do two handles refer to the same node?
    fn same_node(&self, x: Handle, y: Handle) -> bool;

    /// What is the name of this element?
    ///
    /// Should never be called on a non-element node;
    /// feel free to `fail!`.
    fn elem_name(&self, target: Handle) -> (Namespace, Atom);

    /// Set the document's quirks mode.
    fn set_quirks_mode(&mut self, mode: QuirksMode);

    /// Create an element.
    fn create_element(&mut self, ns: Namespace, name: Atom, attrs: Vec<Attribute>) -> Handle;

    /// If the last child of the given element is a text node, append text
    /// to it, otherwise create a new Text node there.
    fn append_text(&mut self, parent: Handle, text: String);

    /// Append a comment as the last child of the given element.
    fn append_comment(&mut self, parent: Handle, text: String);

    /// Append an element as the last child of the given element.
    ///
    /// The child element will not already have a parent.
    fn append_element(&mut self, parent: Handle, child: Handle);

    /// Append a `DOCTYPE` element to the `Document` node.
    fn append_doctype_to_document(&mut self, name: String, public_id: String, system_id: String);

    /// Add each attribute to the given element, if no attribute
    /// with that name already exists.
    fn add_attrs_if_missing(&mut self, target: Handle, attrs: Vec<Attribute>);

    /// Detach the given node from its parent.
    fn remove_from_parent(&mut self, target: Handle);

    /// Mark a HTML `<script>` element as "already started".
    fn mark_script_already_started(&mut self, node: Handle);
}
