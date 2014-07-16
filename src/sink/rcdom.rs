// Copyright 2014 The HTML5 for Rust Project Developers. See the
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

use util::atom::Atom;
use util::namespace::{Namespace, HTML};
use tokenizer::Attribute;
use tree_builder::{TreeSink, QuirksMode, NoQuirks};
use driver::ParseResult;

use std::rc::{Rc, Weak};
use std::cell::RefCell;
use std::default::Default;

/// The different kinds of nodes in the DOM.
#[deriving(Show)]
pub enum NodeEnum {
    /// The `Document` itself.
    Document,

    /// A `DOCTYPE` with name, public id, and system id.
    Doctype(String, String, String),

    /// A text node.
    Text(String),

    /// A comment.
    Comment(String),

    /// An element with attributes.
    ///
    /// FIXME: HTML namespace only for now.
    Element(Atom, Vec<Attribute>),
}

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

    fn parent(&self) -> Handle {
        self.parent.as_ref().expect("no parent!")
            .upgrade().expect("dangling weak pointer!")
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

/// The DOM itself; the result of parsing.
pub struct RcDom {
    /// The `Document` itself.
    pub document: Handle,

    /// Errors that occurred during parsing.
    pub errors: Vec<String>,

    /// The document's quirks mode.
    pub quirks_mode: QuirksMode,
}

impl TreeSink<Handle> for RcDom {
    fn parse_error(&mut self, msg: String) {
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

    fn append_text(&mut self, parent: Handle, text: String) {
        // Append to an existing Text node if we have one.
        match parent.borrow().children.last() {
            Some(h) => match h.borrow_mut().deref_mut().node {
                Text(ref mut existing) => {
                    existing.push_str(text.as_slice());
                    return;
                },
                _ => (),
            },
            _ => (),
        }

        // Otherwise, append a Text node.
        append(&parent, new_node(Text(text)));
    }

    fn append_comment(&mut self, parent: Handle, text: String) {
        append(&parent, new_node(Comment(text)));
    }

    fn append_element(&mut self, parent: Handle, child: Handle) {
        append(&parent, child);
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
        {
            let child = target.borrow();
            let parent = child.parent();
            let mut parent = parent.borrow_mut();
            let (i, _) = parent.children.iter().enumerate()
                .find(|&(_, n)| same_node(n, &target))
                .expect("not found!");
            parent.children.remove(i).expect("not found!");
        }

        let mut child = target.borrow_mut();
        (*child).parent = None;
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
            quirks_mode: NoQuirks,
        }
    }
}

impl ParseResult<RcDom> for RcDom {
    fn get_result(sink: RcDom) -> RcDom {
        sink
    }
}
