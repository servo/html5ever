// Copyright 2015 The xml5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::borrow::Cow::Borrowed;
use tendril::{StrTendril, Tendril};
use tokenizer::{Tag, Pi, QName, Doctype};
use tree_builder::interface::{NodeOrText, TreeSink, AppendNode, AppendText};
use tree_builder::types::{XmlProcessResult, Done};

/// Trait that encapsulates common XML tree actions.
pub trait XmlTreeBuilderActions<Handle> {
    /// Returns current node of in the XmlTreeBuilder.
    fn current_node(&self) -> Handle;

    /// Inserts node or text to its appropriate place in the tree.
    fn insert_appropriately(&mut self, child: NodeOrText<Handle>);

    /// Inserts tag into the tree and adds it to list of open elements.
    fn insert_tag(&mut self, tag: Tag) -> XmlProcessResult;

    /// Appends current tag to the root of the document.
    fn append_tag(&mut self, tag: Tag) -> XmlProcessResult;

    /// Appends tag to the root of the document.
    fn append_tag_to_doc(&mut self, tag: Tag) -> Handle;

    /// Adds element to list of open elements (this should only apply to Tag).
    fn add_to_open_elems(&mut self, el: Handle) -> XmlProcessResult;

    /// Appends comment to root of the document.
    fn append_comment_to_doc(&mut self, comment: StrTendril) -> XmlProcessResult;

    /// Appends comment to the current tag.
    fn append_comment_to_tag(&mut self, text: StrTendril) -> XmlProcessResult;

    /// Appends Doctype to root of the document.
    fn append_doctype_to_doc(&mut self, doctype: Doctype) -> XmlProcessResult;

    /// Appends Processing Instruction to the root of the document
    fn append_pi_to_doc(&mut self, pi: Pi) -> XmlProcessResult;

    /// Appends Processing Instruction to the current tag.
    fn append_pi_to_tag(&mut self, pi: Pi) -> XmlProcessResult;

    /// Appends text to appropriate element.
    fn append_text(&mut self, chars: StrTendril) -> XmlProcessResult;

    /// Checks if given tag is the list of open elements.
    fn tag_in_open_elems(&self, tag: &Tag) -> bool;

    /// Pops elements from list of open elements, until predicate
    /// `pred` returns true
    fn pop_until<TagSet>(&mut self, pred: TagSet) where TagSet: Fn(QName) -> bool;

    /// Checks if current node is in given TagSet
    fn current_node_in<TagSet>(&self, set: TagSet) -> bool where TagSet: Fn(QName) -> bool;

    /// Close given tag.
    fn close_tag(&mut self, tag: Tag) -> XmlProcessResult;

    /// Returns whether or not there are any elements in list of
    /// open elements.
    fn no_open_elems(&self) -> bool;

    /// Removes last element from list of open elements and returns its value.
    fn pop(&mut self) -> Handle ;

    /// Stops parsing of XML file.
    fn stop_parsing(&mut self) -> XmlProcessResult;
}

#[doc(hidden)]
impl<Handle, Sink> XmlTreeBuilderActions<Handle>
    for super::XmlTreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{

    fn current_node(&self) -> Handle {
        self.open_elems.last().expect("no current element").clone()
    }

    fn insert_appropriately(&mut self, child: NodeOrText<Handle>){
        let target = self.current_node();
        self.sink.append(target, child);
    }

    fn insert_tag(&mut self, tag: Tag) -> XmlProcessResult {
        let child = self.sink.create_element(tag.name, tag.attrs);
        self.insert_appropriately(AppendNode(child.clone()));
        self.add_to_open_elems(child)
    }

    fn append_tag(&mut self, tag: Tag) -> XmlProcessResult {
        let child = self.sink.create_element(tag.name, tag.attrs);
        self.insert_appropriately(AppendNode(child));
        Done
    }

    fn append_tag_to_doc(&mut self, tag: Tag) -> Handle {
        let root = self.doc_handle.clone();
        let child = self.sink.create_element(tag.name, tag.attrs);

        self.sink.append(root, AppendNode(child.clone()));
        child
    }

    fn add_to_open_elems(&mut self, el: Handle) -> XmlProcessResult {
        self.open_elems.push(el);

        Done
    }

    fn append_comment_to_doc(&mut self, text: StrTendril) -> XmlProcessResult {
        let target = self.doc_handle.clone();
        let comment = self.sink.create_comment(text);
        self.sink.append(target, AppendNode(comment));
        Done
    }

    fn append_comment_to_tag(&mut self, text: StrTendril) -> XmlProcessResult {
        let target = self.current_node();
        let comment = self.sink.create_comment(text);
        self.sink.append(target, AppendNode(comment));
        Done
    }

    fn append_doctype_to_doc(&mut self, doctype: Doctype) -> XmlProcessResult {
        fn get_tendril(opt: Option<StrTendril>) -> StrTendril {
            match opt {
                Some(expr) => expr,
                None => Tendril::new(),
            }
        };
        self.sink.append_doctype_to_document(
            get_tendril(doctype.name),
            get_tendril(doctype.public_id),
            get_tendril(doctype.system_id),
        );
        Done
    }

    fn append_pi_to_doc(&mut self, pi: Pi) -> XmlProcessResult {
        let target = self.doc_handle.clone();
        let pi = self.sink.create_pi(pi.target, pi.data);
        self.sink.append(target, AppendNode(pi));
        Done
    }

    fn append_pi_to_tag(&mut self, pi: Pi) -> XmlProcessResult {
        let target = self.current_node();
        let pi = self.sink.create_pi(pi.target, pi.data);
        self.sink.append(target, AppendNode(pi));
        Done
    }


    fn append_text(&mut self, chars: StrTendril)
        -> XmlProcessResult {
        self.insert_appropriately(AppendText(chars));
        Done
    }

    fn tag_in_open_elems(&self, tag: &Tag) -> bool {
        self.open_elems
            .iter()
            .any(|a| self.sink.elem_name(a) == tag.name)
    }

    // Pop elements until an element from the set has been popped.  Returns the
    // number of elements popped.
    fn pop_until<P>(&mut self, pred: P)
        where P: Fn(QName) -> bool
    {
        loop {
            if self.current_node_in(|x| pred(x)) {
                break;
            }
            self.open_elems.pop();
            self.namespace_stack.pop();
        }
    }

    fn current_node_in<TagSet>(&self, set: TagSet) -> bool
        where TagSet: Fn(QName) -> bool
    {
        // FIXME: take namespace into consideration:
        set(self.sink.elem_name(&self.current_node()))
    }

    fn close_tag(&mut self, tag: Tag) -> XmlProcessResult {
        debug!("Close tag: current_node.name {:?} \n Current tag {:?}",
                 self.sink.elem_name(&self.current_node()), &tag.name);

        if &self.sink.elem_name(&self.current_node()).local != &tag.name.local {
            self.sink.parse_error(Borrowed("Current node doesn't match tag"));
        }

        let is_closed = self.tag_in_open_elems(&tag);

        if is_closed {
            self.pop_until(|p| p == tag.name);
            self.pop();
        }

        Done
    }

    fn no_open_elems(&self) -> bool {
        self.open_elems.is_empty()
    }

    fn pop(&mut self) -> Handle {
        self.namespace_stack.pop();
        self.open_elems.pop().expect("no current element")
    }

    fn stop_parsing(&mut self) -> XmlProcessResult {
        warn!("stop_parsing for XML5 not implemented, full speed ahead!");
        Done
    }
}
