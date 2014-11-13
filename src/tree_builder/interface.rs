// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The interface for consumers of the tree builder (and thus the
//! parser overall).

use core::prelude::*;

use tokenizer::Attribute;

use collections::vec::Vec;
use collections::str::MaybeOwned;

use string_cache::{QualName, Atom};

use util::span::Span;

pub use self::QuirksMode::{Quirks, LimitedQuirks, NoQuirks};
pub use self::NodeOrText::{AppendNode, AppendText};

/// A document's quirks mode.
#[deriving(PartialEq, Eq, Clone, Hash, Show)]
pub enum QuirksMode {
    Quirks,
    LimitedQuirks,
    NoQuirks,
}

/// Something which can be inserted into the DOM.
///
/// Adjacent sibling text nodes are merged into a single node, so
/// the sink may not want to allocate a `Handle` for each.
pub enum NodeOrText<Handle> {
    AppendNode(Handle),
    AppendText(Span),
}

/// Types which can process tree modifications from the tree builder.
///
/// `Handle` is a reference to a DOM node.  The tree builder requires
/// that a `Handle` implements `Clone` to get another reference to
/// the same node.
pub trait TreeSink<Handle> {
    /// Signal a parse error.
    fn parse_error(&mut self, msg: MaybeOwned<'static>);

    /// Get a handle to the `Document` node.
    fn get_document(&mut self) -> Handle;

    /// Do two handles refer to the same node?
    fn same_node(&self, x: Handle, y: Handle) -> bool;

    /// What is the name of this element?
    ///
    /// Should never be called on a non-element node;
    /// feel free to `panic!`.
    fn elem_name(&self, target: Handle) -> QualName;

    /// Set the document's quirks mode.
    fn set_quirks_mode(&mut self, mode: QuirksMode);

    /// Create an element.
    fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>) -> Handle;

    /// Create a comment node.
    fn create_comment(&mut self, text: Span) -> Handle;

    /// Append a node as the last child of the given node.  If this would
    /// produce adjacent sibling text nodes, it should concatenate the text
    /// instead.
    ///
    /// The child node will not already have a parent.
    fn append(&mut self, parent: Handle, child: NodeOrText<Handle>);

    /// Append a node as the sibling immediately before the given node.  If that node
    /// has no parent, do nothing and return Err(new_node).
    ///
    /// The tree builder promises that `sibling` is not a text node.  However its
    /// old previous sibling, which would become the new node's previous sibling,
    /// could be a text node.  If the new node is also a text node, the two should
    /// be merged, as in the behavior of `append`.
    ///
    /// NB: `new_node` may have an old parent, from which it should be removed.
    fn append_before_sibling(&mut self,
        sibling: Handle,
        new_node: NodeOrText<Handle>) -> Result<(), NodeOrText<Handle>>;

    /// Append a `DOCTYPE` element to the `Document` node.
    fn append_doctype_to_document(&mut self, name: Atom, public_id: Span, system_id: Span);

    /// Add each attribute to the given element, if no attribute
    /// with that name already exists.
    fn add_attrs_if_missing(&mut self, target: Handle, attrs: Vec<Attribute>);

    /// Detach the given node from its parent.
    fn remove_from_parent(&mut self, target: Handle);

    /// Mark a HTML `<script>` element as "already started".
    fn mark_script_already_started(&mut self, node: Handle);

    /// Indicate that a `<script>` element is complete.
    fn complete_script(&mut self, _node: Handle) { }
}

/// Trace hooks for a garbage-collected DOM.
pub trait Tracer<Handle> {
    /// Upon a call to `trace_handles`, the tree builder will call this method
    /// for each handle in its internal state.
    fn trace_handle(&self, node: Handle);
}
