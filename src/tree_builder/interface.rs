// Copyright 2014 The xml5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The interface for consumers of the tree builder (and thus the
//! parser overall).

use tokenizer::{Attribute, QName};

use std::borrow::Cow;

use tendril::StrTendril;

pub use self::NodeOrText::{AppendNode, AppendText};

/// Something which can be inserted into the DOM.
///
/// Adjacent sibling text nodes are merged into a single node, so
/// the sink may not want to allocate a `Handle` for each.
pub enum NodeOrText<Handle> {
    /// Appends next element like it is a node
    AppendNode(Handle),

    /// Appends next element as if it was a text element
    AppendText(StrTendril),
}

/// Whether to interrupt further parsing of the current input until
/// the next explicit resumption of the tokenizer, or continue without
/// any interruption.
#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub enum NextParserState {
    /// Stop further parsing.
    Suspend,
    /// Continue without interruptions.
    Continue,
}

/// Types which can process tree modifications from the tree builder.
pub trait TreeSink {
    /// `Handle` is a reference to a DOM node.  The tree builder requires
    /// that a `Handle` implements `Clone` to get another reference to
    /// the same node.
    type Handle: Clone;

    /// The overall result of parsing.
    ///
    /// TODO:This should defualt to Self, but default associated types are not stable yet.
    /// (https://github.com/rust-lang/rust/issues/29661)
    type Output;

    /// Consume this sink and return the overall result of parsing.
    ///
    /// TODO:This should default to `fn finish(self) -> Self::Output { self }`,
    /// but default associated types are not stable yet.
    /// (https://github.com/rust-lang/rust/issues/29661)
    fn finish(self) -> Self::Output;

    /// Signal a parse error.
    fn parse_error(&mut self, msg: Cow<'static, str>);

    /// Get a handle to the `Document` node.
    fn get_document(&mut self) -> Self::Handle;

    /// What is the name of this element?
    ///
    /// Should never be called on a non-element node;
    /// feel free to `panic!`.
    fn elem_name(&self, target: &Self::Handle) -> QName;

    /// Create an element.
    fn create_element(&mut self, name: QName, attrs: Vec<Attribute>) -> Self::Handle;

    /// Create a comment node.
    fn create_comment(&mut self, text: StrTendril) -> Self::Handle;

    /// Create a Processing Instruction node.
    fn create_pi(&mut self, target: StrTendril, data: StrTendril) -> Self::Handle;

    /// Append a node as the last child of the given node.  If this would
    /// produce adjacent sibling text nodes, it should concatenate the text
    /// instead.
    ///
    /// The child node will not already have a parent.
    fn append(&mut self, parent: Self::Handle, child: NodeOrText<Self::Handle>);

    /// Append a `DOCTYPE` element to the `Document` node.
    fn append_doctype_to_document(&mut self,
                                  name: StrTendril,
                                  public_id: StrTendril,
                                  system_id: StrTendril);

    /// Mark a HTML `<script>` as "already started".
    fn mark_script_already_started(&mut self, _node: Self::Handle) {}

    /// Indicate that a `script` element is complete.
    fn complete_script(&mut self, _node: Self::Handle) -> NextParserState {
        NextParserState::Continue
    }
}

/// Trace hooks for a garbage-collected DOM.
pub trait Tracer {
    /// Reference to a generic DOM node.
    type Handle;

    /// Upon a call to `trace_handles`, the tree builder will call this method
    /// for each handle in its internal state.
    fn trace_handle(&self, node: Self::Handle);
}
