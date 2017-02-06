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

use tokenizer::Attribute;

use std::borrow::Cow;

use QualName;
use tendril::StrTendril;

pub use self::QuirksMode::{Quirks, LimitedQuirks, NoQuirks};
pub use self::NodeOrText::{AppendNode, AppendText};

/// A document's quirks mode.
#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug)]
#[cfg_attr(feature = "heap_size", derive(HeapSizeOf))]
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
    AppendText(StrTendril),
}

/// Types which can process tree modifications from the tree builder.
pub trait TreeSink {
    /// The overall result of parsing.
    ///
    /// This should default to Self, but default associated types are not stable yet.
    /// (https://github.com/rust-lang/rust/issues/29661)
    type Output;

    /// Consume this sink and return the overall result of parsing.
    ///
    /// This should default to `fn finish(self) -> Self::Output { self }`,
    /// but default associated types are not stable yet.
    /// (https://github.com/rust-lang/rust/issues/29661)
    fn finish(self) -> Self::Output;

    /// `Handle` is a reference to a DOM node.  The tree builder requires
    /// that a `Handle` implements `Clone` to get another reference to
    /// the same node.
    type Handle: Clone;

    /// Signal a parse error.
    fn parse_error(&mut self, msg: Cow<'static, str>);

    /// Get a handle to the `Document` node.
    fn get_document(&mut self) -> Self::Handle;

    /// Get a handle to a template's template contents. The tree builder
    /// promises this will never be called with something else than
    /// a template element.
    fn get_template_contents(&mut self, target: Self::Handle) -> Self::Handle;

    /// Do two handles refer to the same node?
    fn same_node(&self, x: Self::Handle, y: Self::Handle) -> bool;

    /// Are two handles present in the same tree
    fn same_tree(&self, x: Self::Handle, y: Self::Handle) -> bool {
        true
    }

    /// What is the name of this element?
    ///
    /// Should never be called on a non-element node;
    /// feel free to `panic!`.
    fn elem_name(&self, target: Self::Handle) -> QualName;

    /// Set the document's quirks mode.
    fn set_quirks_mode(&mut self, mode: QuirksMode);

    /// Create an element.
    ///
    /// When creating a template element (`name == qualname!(html, "template")`),
    /// an associated document fragment called the "template contents" should
    /// also be created. Later calls to self.get_template_contents() with that
    /// given element return it.
    /// https://html.spec.whatwg.org/multipage/#htmltemplateelement
    fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>) -> Self::Handle;

    /// Create a comment node.
    fn create_comment(&mut self, text: StrTendril) -> Self::Handle;

    /// Does the node have a parent?
    fn has_parent_node(&self, node: Self::Handle) -> bool;

    /// Append a node as the last child of the given node.  If this would
    /// produce adjacent sibling text nodes, it should concatenate the text
    /// instead.
    ///
    /// The child node will not already have a parent.
    fn append(&mut self, parent: Self::Handle, child: NodeOrText<Self::Handle>);

    /// Append a node as the sibling immediately before the given node.
    /// This method will only be called if has_parent_node(sibling) is true
    ///
    /// The tree builder promises that `sibling` is not a text node.  However its
    /// old previous sibling, which would become the new node's previous sibling,
    /// could be a text node.  If the new node is also a text node, the two should
    /// be merged, as in the behavior of `append`.
    ///
    /// NB: `new_node` may have an old parent, from which it should be removed.
    fn append_before_sibling(&mut self,
        sibling: Self::Handle,
        new_node: NodeOrText<Self::Handle>);

    /// Append a `DOCTYPE` element to the `Document` node.
    fn append_doctype_to_document(&mut self,
                                  name: StrTendril,
                                  public_id: StrTendril,
                                  system_id: StrTendril);

    /// Add each attribute to the given element, if no attribute with that name
    /// already exists. The tree builder promises this will never be called
    /// with something else than an element.
    fn add_attrs_if_missing(&mut self, target: Self::Handle, attrs: Vec<Attribute>);

    /// Associate the given form-associatable element with the form element
    fn associate_with_form(&mut self, target: Self::Handle, form: Self::Handle) {}

    /// Detach the given node from its parent.
    fn remove_from_parent(&mut self, target: Self::Handle);

    /// Remove all the children from node and append them to new_parent.
    fn reparent_children(&mut self, node: Self::Handle, new_parent: Self::Handle);

    /// Mark a HTML `<script>` element as "already started".
    fn mark_script_already_started(&mut self, node: Self::Handle);

    /// Returns true if the adjusted current node is an HTML integration point
    /// and the token is a start tag.
    fn is_mathml_annotation_xml_integration_point(&self, handle: Self::Handle) -> bool {
        false
    }

    /// Called whenever the line number changes.
    fn set_current_line(&mut self, line_number: u64) {}

    /// Called whenever an element is popped from the stack of open elements.
    fn pop(&mut self, elem: Self::Handle) {}
}

/// Trace hooks for a garbage-collected DOM.
pub trait Tracer {
    type Handle;

    /// Upon a call to `trace_handles`, the tree builder will call this method
    /// for each handle in its internal state.
    fn trace_handle(&self, node: &Self::Handle);
}
