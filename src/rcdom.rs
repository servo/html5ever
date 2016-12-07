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

use std::cell::RefCell;
use std::default::Default;

use std::borrow::Cow;
use std::io::{self, Write};
use std::ops::{Deref, DerefMut};
use std::rc::{Rc, Weak};

use tendril::StrTendril;

pub use self::NodeEnum::{Document, Doctype, Text, Comment, Element, PI};
use super::tokenizer::{Attribute, QName};
use super::tree_builder::{TreeSink, NodeOrText};
use driver::ParseResult;
use serialize::{Serializable, Serializer};
use serialize::{TraversalScope};
use serialize::TraversalScope::{ChildrenOnly, IncludeNode};

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
    Element(QName, Vec<Attribute>),

    /// A Processing instruction.
    PI(StrTendril, StrTendril),
}

/// A simple DOM node.
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
#[derive(Clone)]
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

    fn elem_name(&self, target: &Handle) -> QName {
        // FIXME: rust-lang/rust#22252
        return match target.borrow().node {
            Element(ref name, _) => name.clone(),
            _ => panic!("not an element!"),
        };
    }

    fn create_element(&mut self, name: QName, attrs: Vec<Attribute>) -> Handle {
        new_node(Element(name, attrs))
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
}

impl Default for RcDom {
    fn default() -> RcDom {
        RcDom {
            document: new_node(Document),
            errors: vec!(),
        }
    }
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
            (_, &Element(ref name, ref attrs)) => {
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
