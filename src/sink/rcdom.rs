// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A simple reference-counted DOM.
//!
//! This is sufficient as a static parse tree, but don't build a
//! web browser using it. :)

use core::prelude::*;

use sink::common::{NodeEnum, Document, Doctype, Text, Comment, Element};

use util::namespace::{Namespace, HTML};
use tokenizer::Attribute;
use tree_builder::{TreeSink, QuirksMode, NodeOrText, AppendNode, AppendText};
use tree_builder;
use serialize::{Serializable, Serializer};
use driver::ParseResult;

use core::cell::RefCell;
use core::default::Default;
use alloc::rc::{Rc, Weak};
use collections::MutableSeq;
use collections::vec::Vec;
use collections::string::String;
use collections::str::MaybeOwned;
use std::io::{Writer, IoResult};

use string_cache::Atom;

/// A DOM node.
pub struct Node {
    pub node: NodeEnum,
    pub parent: Option<WeakHandle>,
    pub children: Vec<Handle>,

    /// The "script already started" flag.
    ///
    /// Not meaningful for nodes other than HTML `<script>`.
    pub script_already_started: bool,
}

impl Node {
    fn new(node: NodeEnum) -> Node {
        Node {
            node: node,
            parent: None,
            children: vec!(),
            script_already_started: false,
        }
    }
}

/// Reference to a DOM node.
pub type Handle = Rc<RefCell<Node>>;

/// Weak reference to a DOM node, used for parent pointers.
pub type WeakHandle = Weak<RefCell<Node>>;

fn same_node(x: &Handle, y: &Handle) -> bool {
    // FIXME: This shouldn't really need to touch the borrow flags, right?
    (&*x.borrow() as *const Node) == (&*y.borrow() as *const Node)
}

fn new_node(node: NodeEnum) -> Handle {
    Rc::new(RefCell::new(Node::new(node)))
}

fn append(new_parent: &Handle, child: Handle) {
    new_parent.borrow_mut().children.push(child.clone());
    let parent = &mut child.borrow_mut().parent;
    assert!(parent.is_none());
    *parent = Some(new_parent.downgrade());
}

fn get_parent_and_index(target: &Handle) -> Option<(Handle, uint)> {
    let child = target.borrow();
    let parent = unwrap_or_return!(child.parent.as_ref(), None)
        .upgrade().expect("dangling weak pointer");
    match parent.borrow_mut().children.iter().enumerate()
                .find(|&(_, n)| same_node(n, target)) {
        Some((i, _)) => Some((parent, i)),
        None => fail!("have parent but couldn't find in parent's children!"),
    }
}

fn append_to_existing_text(prev: &Handle, text: &str) -> bool {
    match prev.borrow_mut().deref_mut().node {
        Text(ref mut existing) => {
            existing.push_str(text);
            true
        }
        _ => false,
    }
}

fn remove_from_parent(target: &Handle) {
    {
        let (parent, i) = unwrap_or_return!(get_parent_and_index(target), ());
        parent.borrow_mut().children.remove(i).expect("not found!");
    }

    let mut child = target.borrow_mut();
    (*child).parent = None;
}

/// The DOM itself; the result of parsing.
pub struct RcDom {
    /// The `Document` itself.
    pub document: Handle,

    /// Errors that occurred during parsing.
    pub errors: Vec<MaybeOwned<'static>>,

    /// The document's quirks mode.
    pub quirks_mode: QuirksMode,
}

impl TreeSink<Handle> for RcDom {
    fn parse_error(&mut self, msg: MaybeOwned<'static>) {
        self.errors.push(msg);
    }

    fn get_document(&mut self) -> Handle {
        self.document.clone()
    }

    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        self.quirks_mode = mode;
    }

    fn same_node(&self, x: Handle, y: Handle) -> bool {
        same_node(&x, &y)
    }

    fn elem_name(&self, target: Handle) -> (Namespace, Atom) {
        match target.borrow().node {
            Element(ref name, _) => (HTML, name.clone()),
            _ => fail!("not an element!"),
        }
    }

    fn create_element(&mut self, ns: Namespace, name: Atom, attrs: Vec<Attribute>) -> Handle {
        assert!(ns == HTML);
        new_node(Element(name, attrs))
    }

    fn create_comment(&mut self, text: String) -> Handle {
        new_node(Comment(text))
    }

    fn append(&mut self, parent: Handle, child: NodeOrText<Handle>) {
        // Append to an existing Text node if we have one.
        match child {
            AppendText(ref text) => match parent.borrow().children.last() {
                Some(h) => if append_to_existing_text(h, text.as_slice()) { return; },
                _ => (),
            },
            _ => (),
        }

        append(&parent, match child {
            AppendText(text) => new_node(Text(text)),
            AppendNode(node) => node
        });
    }

    fn append_before_sibling(&mut self,
            sibling: Handle,
            child: NodeOrText<Handle>) -> Result<(), NodeOrText<Handle>> {
        let (parent, i) = unwrap_or_return!(get_parent_and_index(&sibling), Err(child));

        let child = match (child, i) {
            // No previous node.
            (AppendText(text), 0) => new_node(Text(text)),

            // Look for a text node before the insertion point.
            (AppendText(text), i) => {
                let parent = parent.borrow();
                let prev = &parent.children[i-1];
                if append_to_existing_text(prev, text.as_slice()) {
                    return Ok(());
                }
                new_node(Text(text))
            }

            // The tree builder promises we won't have a text node after
            // the insertion point.

            // Any other kind of node.
            (AppendNode(node), _) => node,
        };

        if child.borrow().parent.is_some() {
            remove_from_parent(&child);
        }

        child.borrow_mut().parent = Some(parent.clone().downgrade());
        parent.borrow_mut().children.insert(i, child);
        Ok(())
    }

    fn append_doctype_to_document(&mut self, name: String, public_id: String, system_id: String) {
        append(&self.document, new_node(Doctype(name, public_id, system_id)));
    }

    fn add_attrs_if_missing(&mut self, target: Handle, mut attrs: Vec<Attribute>) {
        let mut node = target.borrow_mut();
        // FIXME: mozilla/rust#15609
        let existing = match node.deref_mut().node {
            Element(_, ref mut attrs) => attrs,
            _ => return,
        };

        // FIXME: quadratic time
        attrs.retain(|attr|
            !existing.iter().any(|e| e.name == attr.name));
        existing.push_all_move(attrs);
    }

    fn remove_from_parent(&mut self, target: Handle) {
        remove_from_parent(&target);
    }

    fn mark_script_already_started(&mut self, node: Handle) {
        node.borrow_mut().script_already_started = true;
    }
}

impl Default for RcDom {
    fn default() -> RcDom {
        RcDom {
            document: new_node(Document),
            errors: vec!(),
            quirks_mode: tree_builder::NoQuirks,
        }
    }
}

impl ParseResult<RcDom> for RcDom {
    fn get_result(sink: RcDom) -> RcDom {
        sink
    }
}

impl Serializable for Handle {
    fn serialize<'wr, Wr: Writer>(&self, serializer: &mut Serializer<'wr, Wr>, incl_self: bool) -> IoResult<()> {
        let node = self.borrow();
        match (incl_self, &node.node) {
            (_, &Element(ref name, ref attrs)) => {
                if incl_self {
                    try!(serializer.start_elem(HTML, name.clone(),
                        attrs.iter().map(|at| (&at.name, at.value.as_slice()))));
                }

                for handle in node.children.iter() {
                    try!(handle.clone().serialize(serializer, true));
                }

                if incl_self {
                    try!(serializer.end_elem(HTML, name.clone()));
                }
                Ok(())
            }

            (false, &Document) => {
                for handle in node.children.iter() {
                    try!(handle.clone().serialize(serializer, true));
                }
                Ok(())
            }

            (false, _) => Ok(()),

            (true, &Doctype(ref name, _, _)) => serializer.write_doctype(name.as_slice()),
            (true, &Text(ref text)) => serializer.write_text(text.as_slice()),
            (true, &Comment(ref text)) => serializer.write_comment(text.as_slice()),

            (true, &Document) => fail!("Can't serialize Document node itself"),
        }
    }
}
