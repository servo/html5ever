// Copyright 2014-2017 The html5ever Project Developers. See the
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

use std::cell::RefCell;
use std::default::Default;
use std::borrow::Cow;
use std::io::{self, Write};
use std::ops::{Deref, DerefMut};
use std::rc::{Rc, Weak};
use std::collections::HashSet;
use std::mem;

use tendril::StrTendril;
use markup5ever::interface::{Attribute, QualName, QuirksMode, AppendText, AppendNode};

use super::tree_builder::{TreeSink, NodeOrText};
use serialize::{Serializable, Serializer};
use serialize::{TraversalScope};
use serialize::TraversalScope::{ChildrenOnly, IncludeNode};

pub use self::NodeEnum::{Document, Doctype, Text, Comment, Element, PI};
pub use self::ElementEnum::{Normal, Script, Template};


/// The different kinds of elements in the DOM.
#[derive(Debug)]
pub enum ElementEnum {
    /// Regular element.
    Normal,
    /// A script element
    Script{
        /// Script element's already-started flag
        /// https://html.spec.whatwg.org/multipage/#already-started
        script_already_started: bool
    },
    /// A template element and its template contents.
    /// https://html.spec.whatwg.org/multipage/#template-contents
    Template(Handle),
}


/// The different kinds of nodes in the DOM.
#[derive(Debug)]
pub enum NodeEnum {
    /// The `Document` itself.
    Document,

    /// A `DOCTYPE` with name, public id, and system id.
    Doctype(StrTendril, StrTendril, StrTendril),

    /// A text node.
    Text(StrTendril),

    /// A comment.
    Comment(StrTendril),

    /// An element with attributes.
    Element(QualName, ElementEnum, Vec<Attribute>),

    /// A Processing instruction.
    PI(StrTendril, StrTendril),
}

/// A simple DOM node.
#[derive(Debug)]
pub struct Node {
    /// Represents this node's data.
    pub node: NodeEnum,
    /// Parent node.
    pub parent: Option<WeakHandle>,
    /// Child nodes of this node.
    pub children: Vec<Handle>,
}

impl Node {
    fn new(node: NodeEnum) -> Node {
        Node {
            node: node,
            parent: None,
            children: vec!(),
        }
    }
}

/// Reference to a DOM node.
#[derive(Clone, Debug)]
pub struct Handle(Rc<RefCell<Node>>);

impl Deref for Handle {
    type Target = Rc<RefCell<Node>>;
    fn deref(&self) -> &Rc<RefCell<Node>> { &self.0 }
}

/// Weak reference to a DOM node, used for parent pointers.
pub type WeakHandle = Weak<RefCell<Node>>;

fn new_node(node: NodeEnum) -> Handle {
    Handle(Rc::new(RefCell::new(Node::new(node))))
}

fn append(new_parent: &Handle, child: Handle) {
    new_parent.borrow_mut().children.push(child.clone());
    let parent = &mut child.borrow_mut().parent;
    assert!(parent.is_none());
    *parent = Some(Rc::downgrade(new_parent));
}

fn append_to_existing_text(prev: &Handle, text: &str) -> bool {
    match prev.borrow_mut().deref_mut().node {
        Text(ref mut existing) => {
            existing.push_slice(text);
            true
        }
        _ => false,
    }
}

fn get_parent_and_index(target: &Handle) -> Option<(Handle, usize)> {
    let child = target.borrow();
    let parent = unwrap_or_return!(child.parent.as_ref(), None)
        .upgrade().expect("dangling weak pointer");

    let i = match parent.borrow_mut().children.iter().enumerate()
                .find(|&(_, child)| Rc::ptr_eq(&child.0, &target.0)) {
        Some((i, _)) => i,
        None => panic!("have parent but couldn't find in parent's children!"),
    };
    Some((Handle(parent), i))
}

fn remove_from_parent(target: &Handle) {
    {
        let (parent, i) = unwrap_or_return!(get_parent_and_index(target), ());
        parent.borrow_mut().children.remove(i);
    }

    let mut child = target.borrow_mut();
    (*child).parent = None;
}



/// The DOM itself; the result of parsing.
pub struct RcDom {
    /// The `Document` itself.
    pub document: Handle,

    /// Errors that occurred during parsing.
    pub errors: Vec<Cow<'static, str>>,
}

impl TreeSink for RcDom {
    type Handle = Handle;
    type Output = Self;

    fn finish(self) -> Self::Output {
        self
    }

    fn parse_error(&mut self, msg: Cow<'static, str>) {
        self.errors.push(msg);
    }

    fn get_document(&mut self) -> Handle {
        self.document.clone()
    }

    fn elem_name(&self, target: Handle) -> QualName {
        // FIXME: rust-lang/rust#22252
        if let Element(ref name, _, _) = target.borrow().node {
            name.clone()
        } else {
            panic!("not an element!")
        }
    }

    fn elem_name_ref(&self, target: &Handle) -> QualName {
        // FIXME: rust-lang/rust#22252
        return match target.borrow().node {
            Element(ref name, _, _) => name.clone(),
            _ => panic!("not an element!"),
        };
    }

    fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>) -> Handle {
        new_node(Element(name, Normal, attrs))
    }

    fn create_comment(&mut self, text: StrTendril) -> Handle {
        new_node(Comment(text))
    }

    fn create_pi(&mut self, target: StrTendril, data: StrTendril) -> Handle {
        new_node(PI(target, data))
    }

    fn append(&mut self, parent: Handle, child: NodeOrText<Handle>) {
        // Append to an existing Text node if we have one.
        match child {
            NodeOrText::AppendText(ref text) => match parent.borrow().children.last() {
                Some(h) => if append_to_existing_text(h, &text) { return; },
                _ => (),
            },
            _ => (),
        }

        append(&parent, match child {
            NodeOrText::AppendText(text) => new_node(Text(text)),
            NodeOrText::AppendNode(node) => node
        });
    }

    fn append_doctype_to_document(&mut self,
                                  name: StrTendril,
                                  public_id: StrTendril,
                                  system_id: StrTendril) {
        append(&self.document, new_node(Doctype(name, public_id, system_id)));
    }

    fn mark_script_already_started(&mut self, target: Handle) {
        if let Element(_, Script {ref mut script_already_started}, _) = target.borrow_mut().node {
            *script_already_started = true;
        } else {
            panic!("not a script element!");
        }
    }

    fn get_template_contents(&mut self, target: Handle) -> Handle {
        if let Element(_, Template(ref contents), _) = target.borrow().node {
            contents.clone()
        } else {
            panic!("not a template element!")
        }
    }

    fn same_node(&self, x: Handle, y: Handle) -> bool {
        self.same_node_ref(&x, &y)
    }

    fn same_node_ref(&self, x: &Handle, y: &Handle) -> bool {
        Rc::ptr_eq(x, y)
    }


    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        // XML doesn't have quirks mode
    }

    fn has_parent_node(&self, node: Handle) -> bool {
        let node = node.borrow();
        node.parent.is_some()
    }

    fn append_before_sibling(&mut self,
            sibling: Handle,
            child: NodeOrText<Handle>) {
        let (parent, i) = get_parent_and_index(&sibling)
            .expect("append_before_sibling called on node without parent");

        let child = match (child, i) {
            // No previous node.
            (AppendText(text), 0) => new_node(Text(text)),

            // Look for a text node before the insertion point.
            (AppendText(text), i) => {
                let parent = parent.borrow();
                let prev = &parent.children[i-1];
                if append_to_existing_text(prev, &text) {
                    return;
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

        child.borrow_mut().parent = Some(Rc::downgrade(&parent));
        parent.borrow_mut().children.insert(i, child);
    }

    fn add_attrs_if_missing(&mut self, target: Handle, attrs: Vec<Attribute>) {
        let mut node = target.borrow_mut();
        let existing = if let Element(_, _, ref mut attrs) = node.deref_mut().node {
            attrs
        } else {
            panic!("not an element")
        };

        let existing_names =
            existing.iter().map(|e| e.name.clone()).collect::<HashSet<_>>();
        existing.extend(attrs.into_iter().filter(|attr| {
            !existing_names.contains(&attr.name)
        }));
    }

    fn remove_from_parent(&mut self, target: Handle) {
        remove_from_parent(&target);
    }

    fn reparent_children(&mut self, node: Handle, new_parent: Handle) {
        let children = &mut node.borrow_mut().children;
        let new_children = &mut new_parent.borrow_mut().children;
        for child in children.iter() {
            // FIXME: It would be nice to assert that the child's parent is node, but I haven't
            // found a way to do that that doesn't create overlapping borrows of RefCells.
            let parent = &mut child.borrow_mut().parent;
            *parent = Some(Rc::downgrade(&new_parent));
        }
        new_children.extend(mem::replace(children, Vec::new()).into_iter());
    }

}

impl Default for RcDom {
    fn default() -> RcDom {
        RcDom {
            document: new_node(Document),
            errors: vec!(),
        }
    }
}

/// Results which can be extracted from a `TreeSink`.
///
/// Implement this for your parse tree data type so that it
/// can be returned by `parse()`.
pub trait ParseResult {
    /// Type of consumer of tree modifications.
    /// It also extends `Default` for convenience.
    type Sink: TreeSink + Default;
    /// Returns parsed tree data type
    fn get_result(sink: Self::Sink) -> Self;
}

impl ParseResult for RcDom {
    type Sink = RcDom;

    fn get_result(sink: RcDom) -> RcDom {
        sink
    }
}

impl Serializable for Handle {
    fn serialize<'wr, Wr>(&self, serializer: &mut Serializer<'wr, Wr>,
                            traversal_scope: TraversalScope) -> io::Result<()>
        where Wr: Write {

        let node = self.borrow();
        match (traversal_scope, &node.node) {
            (_, &Element(ref name, _, ref attrs)) => {
                if traversal_scope == IncludeNode {
                    try!(serializer.start_elem(name.clone(),
                        attrs.iter().map(|at| (&at.name, &at.value[..]))));
                }

                for handle in node.children.iter() {
                    try!(handle.clone().serialize(serializer, IncludeNode));
                }

                if traversal_scope == IncludeNode {
                    try!(serializer.end_elem(name.clone()));
                }
                Ok(())
            }

            (ChildrenOnly, &Document) => {
                for handle in node.children.iter() {
                    try!(handle.clone().serialize(serializer, IncludeNode));
                }
                Ok(())
            }

            (ChildrenOnly, _) => Ok(()),

            (IncludeNode, &Doctype(ref name, _, _)) => serializer.write_doctype(&name),
            (IncludeNode, &Text(ref text)) => serializer.write_text(&text),
            (IncludeNode, &Comment(ref text)) => serializer.write_comment(&text),
            (IncludeNode, &PI(ref target, ref data)) => {
                serializer.write_processing_instruction(&target, data)
            },
            (IncludeNode, &Document) => panic!("Can't serialize Document node itself"),
        }
    }
}
