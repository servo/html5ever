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

use std::ascii::AsciiExt;
use std::cell::{RefCell, Cell};
use std::collections::HashSet;
use std::default::Default;
use std::borrow::Cow;
use std::io;
use std::mem;
use std::ops::Deref;
use std::rc::{Rc, Weak};

use tendril::StrTendril;

use Attribute;
use QualName;
use interface::tree_builder::{TreeSink, QuirksMode, NodeOrText, AppendNode, AppendText};
use interface::tree_builder;
use serialize::{Serialize, Serializer};
use serialize::TraversalScope;
use serialize::TraversalScope::{IncludeNode, ChildrenOnly};

pub use self::ElementEnum::{AnnotationXml, Normal, Script, Template};
pub use self::NodeEnum::{Document, Doctype, Text, Comment, Element, PI};

/// The different kinds of elements in the DOM.
pub enum ElementEnum {
    Normal,
    /// A script element and its "already started" flag.
    /// https://html.spec.whatwg.org/multipage/#already-started
    Script(Cell<bool>),
    /// A template element and its template contents.
    /// https://html.spec.whatwg.org/multipage/#template-contents
    Template(Handle),
    /// An annotation-xml element in the MathML namespace whose start tag token had an attribute
    /// with the name "encoding" whose value was an ASCII case-insensitive match for the string
    /// "text/html" or "application/xhtml+xml"
    /// https://html.spec.whatwg.org/multipage/embedded-content.html#math:annotation-xml
    AnnotationXml(bool),
}

/// The different kinds of nodes in the DOM.
pub enum NodeEnum {
    /// The `Document` itself.
    Document,

    /// A `DOCTYPE` with name, public id, and system id.
    Doctype(StrTendril, StrTendril, StrTendril),

    /// A text node.
    Text(RefCell<StrTendril>),

    /// A comment.
    Comment(StrTendril),

    /// An element with attributes.
    Element(QualName, ElementEnum, RefCell<Vec<Attribute>>),

    /// A Processing instruction.
    PI(StrTendril, StrTendril),
}

/// A DOM node.
pub struct Node {
    /// Represents this node's data.
    pub node: NodeEnum,
    /// Parent node.
    pub parent: Cell<Option<WeakHandle>>,
    /// Child nodes of this node.
    pub children: RefCell<Vec<Handle>>,
}

impl Node {
    fn new(node: NodeEnum) -> Node {
        Node {
            node: node,
            parent: Cell::new(None),
            children: RefCell::new(Vec::new()),
        }
    }
}

/// Reference to a DOM node.
#[derive(Clone)]
pub struct Handle(Rc<Node>);

impl Deref for Handle {
    type Target = Rc<Node>;
    fn deref(&self) -> &Rc<Node> { &self.0 }
}

/// Weak reference to a DOM node, used for parent pointers.
pub type WeakHandle = Weak<Node>;

fn new_node(node: NodeEnum) -> Handle {
    Handle(Rc::new(Node::new(node)))
}

fn append(new_parent: &Handle, child: Handle) {
    let previous_parent = child.parent.replace(Some(Rc::downgrade(new_parent)));
    assert!(previous_parent.is_none());
    new_parent.children.borrow_mut().push(child);
}

fn get_parent_and_index(target: &Handle) -> Option<(Handle, usize)> {
    if let Some(weak) = target.parent.take() {
        let parent = weak.upgrade().expect("dangling weak pointer");
        target.parent.set(Some(weak));
        let i = match parent.children.borrow().iter().enumerate()
                    .find(|&(_, child)| Rc::ptr_eq(&child.0, &target.0)) {
            Some((i, _)) => i,
            None => panic!("have parent but couldn't find in parent's children!"),
        };
        Some((Handle(parent), i))
    } else {
        None
    }
}

fn append_to_existing_text(prev: &Handle, text: &str) -> bool {
    match prev.node {
        Text(ref existing) => {
            existing.borrow_mut().push_slice(text);
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
        if let Element(_, Template(ref contents), _) = target.node {
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

    fn elem_name(&self, target: &Handle) -> QualName {
        return match target.node {
            Element(ref name, _, _) => name.clone(),
            _ => panic!("not an element!"),
        };
    }

    fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>) -> Handle {
        let info = match name {
            qualname!(html, "script") => Script(Cell::new(false)),
            qualname!(html, "template") => Template(new_node(Document)),
            qualname!(mathml, "annotation-xml") => {
                AnnotationXml(attrs.iter().find(|attr| attr.name == qualname!("", "encoding"))
                                   .map_or(false,
                                           |attr| attr.value
                                                      .eq_ignore_ascii_case("text/html") ||
                                                  attr.value
                                                      .eq_ignore_ascii_case("application/xhtml+xml")))
            },
            _ => Normal,
        };
        new_node(Element(name, info, RefCell::new(attrs)))
    }

    fn create_comment(&mut self, text: StrTendril) -> Handle {
        new_node(Comment(text))
    }

    fn create_pi(&mut self, target: StrTendril, data: StrTendril) -> Handle {
        new_node(PI(target, data))
    }

    fn has_parent_node(&self, node: &Handle) -> bool {
        let parent = node.parent.take();
        let has_parent = parent.is_some();
        node.parent.set(parent);
        has_parent
    }

    fn append(&mut self, parent: &Handle, child: NodeOrText<Handle>) {
        // Append to an existing Text node if we have one.
        match child {
            AppendText(ref text) => match parent.children.borrow().last() {
                Some(h) => if append_to_existing_text(h, &text) { return; },
                _ => (),
            },
            _ => (),
        }

        append(&parent, match child {
            AppendText(text) => new_node(Text(RefCell::new(text))),
            AppendNode(node) => node
        });
    }

    fn append_before_sibling(&mut self,
            sibling: &Handle,
            child: NodeOrText<Handle>) {
        let (parent, i) = get_parent_and_index(&sibling)
            .expect("append_before_sibling called on node without parent");

        let child = match (child, i) {
            // No previous node.
            (AppendText(text), 0) => new_node(Text(RefCell::new(text))),

            // Look for a text node before the insertion point.
            (AppendText(text), i) => {
                let children = parent.children.borrow();
                let prev = &children[i-1];
                if append_to_existing_text(prev, &text) {
                    return;
                }
                new_node(Text(RefCell::new(text)))
            }

            // The tree builder promises we won't have a text node after
            // the insertion point.

            // Any other kind of node.
            (AppendNode(node), _) => node,
        };

        remove_from_parent(&child);

        child.parent.set(Some(Rc::downgrade(&parent)));
        parent.children.borrow_mut().insert(i, child);
    }

    fn append_doctype_to_document(&mut self,
                                  name: StrTendril,
                                  public_id: StrTendril,
                                  system_id: StrTendril) {
        append(&self.document, new_node(Doctype(name, public_id, system_id)));
    }

    fn add_attrs_if_missing(&mut self, target: &Handle, attrs: Vec<Attribute>) {
        let mut existing = if let Element(_, _, ref attrs) = target.node {
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

    fn mark_script_already_started(&mut self, target: &Handle) {
        if let Element(_, Script(ref script_already_started), _) = target.node {
            script_already_started.set(true);
        } else {
            panic!("not a script element!");
        }
    }

    fn is_mathml_annotation_xml_integration_point(&self, handle: &Handle) -> bool {
        match handle.node {
            Element(_, AnnotationXml(ret), _) => ret,
            _ => unreachable!(),
        }
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

impl Serialize for Handle {
    fn serialize<S>(&self, serializer: &mut S, traversal_scope: TraversalScope) -> io::Result<()>
    where S: Serializer {
        match (traversal_scope, &self.node) {
            (_, &Element(ref name, _, ref attrs)) => {
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

            (ChildrenOnly, &Document) => {
                for handle in self.children.borrow().iter() {
                    try!(handle.clone().serialize(serializer, IncludeNode));
                }
                Ok(())
            }

            (ChildrenOnly, _) => Ok(()),

            (IncludeNode, &Doctype(ref name, _, _)) => serializer.write_doctype(&name),
            (IncludeNode, &Text(ref text)) => serializer.write_text(&*text.borrow()),
            (IncludeNode, &Comment(ref text)) => serializer.write_comment(&text),
            (IncludeNode, &PI(ref target, ref data)) => {
                serializer.write_processing_instruction(&target, data)
            },
            (IncludeNode, &Document) => panic!("Can't serialize Document node itself"),
        }
    }
}
