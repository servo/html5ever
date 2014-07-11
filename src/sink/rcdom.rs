// Copyright 2014 The HTML5 for Rust Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use util::atom::Atom;
use util::namespace::{Namespace, HTML};
use tokenizer::Attribute;
use tree_builder::{TreeSink, QuirksMode, NoQuirks};
use driver::ParseResult;

use std::rc::{Rc, Weak};
use std::cell::RefCell;
use std::default::Default;

#[deriving(Show)]
pub enum NodeEnum {
    Document,
    Doctype(String, String, String),
    Text(String),
    Comment(String),
    Element(Atom, Vec<Attribute>),
}

pub struct Node {
    pub node: NodeEnum,
    pub parent: Option<WeakHandle>,
    pub children: Vec<Handle>,
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
        self.parent.as_ref().expect("no parent!").upgrade()
    }
}

#[deriving(Clone)]
pub struct Handle {
    ptr: Rc<RefCell<Node>>,
}

impl Handle {
    fn new(node: NodeEnum) -> Handle {
        Handle {
            ptr: Rc::new(RefCell::new(Node::new(node))),
        }
    }

    pub fn downgrade(&self) -> WeakHandle {
        WeakHandle {
            ptr: self.ptr.downgrade(),
        }
    }

    pub fn append(&self, child: Handle) {
        self.borrow_mut().children.push(child.clone());
        let parent = &mut child.borrow_mut().parent;
        assert!(parent.is_none());
        *parent = Some(self.downgrade());
    }
}

// Implement an object-identity Eq for use by position_elem().
impl PartialEq for Handle {
    fn eq(&self, other: &Handle) -> bool {
        // FIXME: This shouldn't really need to touch the borrow flags, right?
        (&*self.ptr.borrow() as *const Node) == (&*other.ptr.borrow() as *const Node)
    }
}

impl Eq for Handle { }

impl Deref<Rc<RefCell<Node>>> for Handle {
    fn deref<'a>(&'a self) -> &'a Rc<RefCell<Node>> {
        &self.ptr
    }
}

pub struct WeakHandle {
    ptr: Weak<RefCell<Node>>,
}

impl WeakHandle {
    pub fn upgrade(&self) -> Handle {
        Handle {
            ptr: self.ptr.upgrade().expect("dangling weak pointer!"),
        }
    }
}

pub struct RcDom {
    pub document: Handle,
    pub root: Option<Handle>,
    pub errors: Vec<String>,
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
        x == y
    }

    fn elem_name(&self, target: Handle) -> (Namespace, Atom) {
        match target.ptr.borrow().node {
            Element(ref name, _) => (HTML, name.clone()),
            _ => fail!("not an element!"),
        }
    }

    fn create_element(&mut self, ns: Namespace, name: Atom, attrs: Vec<Attribute>) -> Handle {
        assert!(ns == HTML);
        Handle::new(Element(name, attrs))
    }

    fn append_text(&mut self, parent: Handle, text: String) {
        parent.append(Handle::new(Text(text)));
    }

    fn append_comment(&mut self, parent: Handle, text: String) {
        parent.append(Handle::new(Comment(text)));
    }

    fn append_element(&mut self, parent: Handle, child: Handle) {
        parent.append(child);
    }

    fn append_doctype_to_document(&mut self, name: String, public_id: String, system_id: String) {
        self.document.append(Handle::new(Doctype(name, public_id, system_id)));
    }

    fn add_attrs_if_missing(&mut self, target: Handle, mut attrs: Vec<Attribute>) {
        let mut node = target.ptr.borrow_mut();
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
            let i = parent.children.as_slice().position_elem(&target).expect("not found!");
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
            document: Handle::new(Document),
            root: None,
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
