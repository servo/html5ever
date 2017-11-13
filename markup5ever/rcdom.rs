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

use std::cell::{RefCell, Cell};
use std::collections::HashSet;
use std::default::Default;
use std::borrow::Cow;
use std::io;
use std::mem;
use std::rc::{Rc, Weak};

use tendril::StrTendril;

use Attribute;
use ExpandedName;
use QualName;
use interface::tree_builder::{TreeSink, QuirksMode, NodeOrText, ElementFlags};
use interface::tree_builder;
use serialize::{Serialize, Serializer};
use serialize::TraversalScope;
use serialize::TraversalScope::{IncludeNode, ChildrenOnly};

/// The different kinds of nodes in the DOM.
pub enum NodeData {
    /// The `Document` itself.
    Document,

    /// A `DOCTYPE` with name, public id, and system id.
    Doctype {
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    },

    /// A text node.
    Text {
        contents: RefCell<StrTendril>,
    },

    /// A comment.
    Comment {
        contents: StrTendril,
    },

    /// An element with attributes.
    Element {
        name: QualName,
        attrs: RefCell<Vec<Attribute>>,

        /// For HTML <template> elements, the template contents
        /// https://html.spec.whatwg.org/multipage/#template-contents
        template_contents: Option<Handle>,

        /// https://html.spec.whatwg.org/multipage/#html-integration-point
        mathml_annotation_xml_integration_point: bool,
    },

    /// A Processing instruction.
    ProcessingInstruction {
        target: StrTendril,
        contents: StrTendril,
    },
}

/// A DOM node.
pub struct Node {
    /// Parent node.
    pub parent: Cell<Option<WeakHandle>>,
    /// Child nodes of this node.
    pub children: RefCell<Vec<Handle>>,
    /// Represents this node's data.
    pub data: NodeData,
}

impl Node {
    /// Create a new node from its contents
    fn new(data: NodeData) -> Rc<Self> {
        Rc::new(Node {
            data: data,
            parent: Cell::new(None),
            children: RefCell::new(Vec::new()),
        })
    }
}

/// Reference to a DOM node.
pub type Handle = Rc<Node>;

/// Weak reference to a DOM node, used for parent pointers.
pub type WeakHandle = Weak<Node>;

/// Append a parentless node to another nodes' children
fn append(new_parent: &Handle, child: Handle) {
    let previous_parent = child.parent.replace(Some(Rc::downgrade(new_parent)));
    // Invariant: child cannot have existing parent
    assert!(previous_parent.is_none());
    new_parent.children.borrow_mut().push(child);
}

/// If the node has a parent, get it and this node's position in its children
fn get_parent_and_index(target: &Handle) -> Option<(Handle, usize)> {
    if let Some(weak) = target.parent.take() {
        let parent = weak.upgrade().expect("dangling weak pointer");
        target.parent.set(Some(weak));
        let i = match parent.children.borrow().iter().enumerate()
                    .find(|&(_, child)| Rc::ptr_eq(&child, &target)) {
            Some((i, _)) => i,
            None => panic!("have parent but couldn't find in parent's children!"),
        };
        Some((parent, i))
    } else {
        None
    }
}

fn append_to_existing_text(prev: &Handle, text: &str) -> bool {
    match prev.data {
        NodeData::Text { ref contents } => {
            contents.borrow_mut().push_slice(text);
            true
        }
        _ => false,
    }
}

fn remove_from_parent(target: &Handle) {
    if let Some((parent, i)) = get_parent_and_index(target) {
        parent.children.borrow_mut().remove(i);
        target.parent.set(None);
    }
}

/// The DOM itself; the result of parsing.
pub struct RcDom {
    /// The `Document` itself.
    pub document: Handle,

    /// Errors that occurred during parsing.
    pub errors: Vec<Cow<'static, str>>,

    /// The document's quirks mode.
    pub quirks_mode: QuirksMode,
}

impl TreeSink for RcDom {
    type Output = Self;
    fn finish(self) -> Self { self }

    type Handle = Handle;

    fn parse_error(&mut self, msg: Cow<'static, str>) {
        self.errors.push(msg);
    }

    fn get_document(&mut self) -> Handle {
        self.document.clone()
    }

    fn get_template_contents(&mut self, target: &Handle) -> Handle {
        if let NodeData::Element { template_contents: Some(ref contents), .. } = target.data {
            contents.clone()
        } else {
            panic!("not a template element!")
        }
    }

    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        self.quirks_mode = mode;
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        Rc::ptr_eq(x, y)
    }

    fn elem_name<'a>(&self, target: &'a Handle) -> ExpandedName<'a> {
        return match target.data {
            NodeData::Element { ref name, .. } => name.expanded(),
            _ => panic!("not an element!"),
        };
    }

    fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        Node::new(NodeData::Element {
            name: name,
            attrs: RefCell::new(attrs),
            template_contents: if flags.template {
                Some(Node::new(NodeData::Document))
            } else {
                None
            },
            mathml_annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
        })
    }

    fn create_comment(&mut self, text: StrTendril) -> Handle {
        Node::new(NodeData::Comment { contents: text })
    }

    fn create_pi(&mut self, target: StrTendril, data: StrTendril) -> Handle {
        Node::new(NodeData::ProcessingInstruction { target: target, contents: data })
    }

    fn append(&mut self, parent: &Handle, child: NodeOrText<Handle>) {
        // Append to an existing Text node if we have one.
        match child {
            NodeOrText::AppendText(ref text) => match parent.children.borrow().last() {
                Some(h) => if append_to_existing_text(h, &text) { return; },
                _ => (),
            },
            _ => (),
        }

        append(&parent, match child {
            NodeOrText::AppendText(text) => Node::new(NodeData::Text { contents: RefCell::new(text) }),
            NodeOrText::AppendNode(node) => node
        });
    }

    fn append_before_sibling(&mut self,
            sibling: &Handle,
            child: NodeOrText<Handle>) {
        let (parent, i) = get_parent_and_index(&sibling)
            .expect("append_before_sibling called on node without parent");

        let child = match (child, i) {
            // No previous node.
            (NodeOrText::AppendText(text), 0) => {
                Node::new(NodeData::Text { contents: RefCell::new(text) })
            }

            // Look for a text node before the insertion point.
            (NodeOrText::AppendText(text), i) => {
                let children = parent.children.borrow();
                let prev = &children[i-1];
                if append_to_existing_text(prev, &text) {
                    return;
                }
                Node::new(NodeData::Text { contents: RefCell::new(text) })
            }

            // The tree builder promises we won't have a text node after
            // the insertion point.

            // Any other kind of node.
            (NodeOrText::AppendNode(node), _) => node,
        };

        remove_from_parent(&child);

        child.parent.set(Some(Rc::downgrade(&parent)));
        parent.children.borrow_mut().insert(i, child);
    }

    fn append_based_on_parent_node(&mut self,
        element: &Self::Handle,
        prev_element: &Self::Handle,
        child: NodeOrText<Self::Handle>) {
        let parent = element.parent.take();
        let has_parent = parent.is_some();
        element.parent.set(parent);

        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(&mut self,
                                  name: StrTendril,
                                  public_id: StrTendril,
                                  system_id: StrTendril) {
        append(&self.document, Node::new(NodeData::Doctype {
            name: name,
            public_id: public_id,
            system_id: system_id
        }));
    }

    fn add_attrs_if_missing(&mut self, target: &Handle, attrs: Vec<Attribute>) {
        let mut existing = if let NodeData::Element { ref attrs, .. } = target.data {
            attrs.borrow_mut()
        } else {
            panic!("not an element")
        };

        let existing_names =
            existing.iter().map(|e| e.name.clone()).collect::<HashSet<_>>();
        existing.extend(attrs.into_iter().filter(|attr| {
            !existing_names.contains(&attr.name)
        }));
    }

    fn remove_from_parent(&mut self, target: &Handle) {
        remove_from_parent(&target);
    }

    fn reparent_children(&mut self, node: &Handle, new_parent: &Handle) {
        let mut children = node.children.borrow_mut();
        let mut new_children = new_parent.children.borrow_mut();
        for child in children.iter() {
            let previous_parent = child.parent.replace(Some(Rc::downgrade(&new_parent)));
            assert!(Rc::ptr_eq(&node, &previous_parent.unwrap().upgrade().expect("dangling weak")))
        }
        new_children.extend(mem::replace(&mut *children, Vec::new()));
    }

    fn is_mathml_annotation_xml_integration_point(&self, target: &Handle) -> bool {
        if let NodeData::Element { mathml_annotation_xml_integration_point, .. } = target.data {
            mathml_annotation_xml_integration_point
        } else {
            panic!("not an element!")
        }
    }
}

impl Default for RcDom {
    fn default() -> RcDom {
        RcDom {
            document: Node::new(NodeData::Document),
            errors: vec!(),
            quirks_mode: tree_builder::NoQuirks,
        }
    }
}

impl Serialize for Handle {
    fn serialize<S>(&self, serializer: &mut S, traversal_scope: TraversalScope) -> io::Result<()>
    where S: Serializer {
        match (&traversal_scope, &self.data) {
            (_, &NodeData::Element { ref name, ref attrs, .. }) => {
                if traversal_scope == IncludeNode {
                    try!(serializer.start_elem(name.clone(),
                        attrs.borrow().iter().map(|at| (&at.name, &at.value[..]))));
                }

                for handle in self.children.borrow().iter() {
                    try!(handle.clone().serialize(serializer, IncludeNode));
                }

                if traversal_scope == IncludeNode {
                    try!(serializer.end_elem(name.clone()));
                }
                Ok(())
            }

            (&ChildrenOnly(_), &NodeData::Document) => {
                for handle in self.children.borrow().iter() {
                    try!(handle.clone().serialize(serializer, IncludeNode));
                }
                Ok(())
            }

            (&ChildrenOnly(_), _) => Ok(()),

            (&IncludeNode, &NodeData::Doctype { ref name, .. }) => {
                serializer.write_doctype(&name)
            }
            (&IncludeNode, &NodeData::Text { ref contents }) => {
                serializer.write_text(&contents.borrow())
            }
            (&IncludeNode, &NodeData::Comment { ref contents }) => {
                serializer.write_comment(&contents)
            }
            (&IncludeNode, &NodeData::ProcessingInstruction { ref target, ref contents }) => {
                serializer.write_processing_instruction(target, contents)
            },
            (&IncludeNode, &NodeData::Document) => {
                panic!("Can't serialize Document node itself")
            }
        }
    }
}
