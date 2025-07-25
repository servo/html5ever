// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod types;

use log::{debug, warn};
use markup5ever::{local_name, namespace_prefix, ns};
use std::borrow::Cow;
use std::borrow::Cow::Borrowed;
use std::cell::{Cell, Ref, RefCell};
use std::collections::btree_map::Iter;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fmt::{Debug, Error, Formatter};
use std::mem;

pub use self::interface::{ElemName, NodeOrText, Tracer, TreeSink};
use self::types::*;
use crate::interface::{self, create_element, AppendNode, Attribute, QualName};
use crate::interface::{AppendText, ExpandedName};
use crate::tokenizer::{self, EndTag, ProcessResult, StartTag, Tag, TokenSink};
use crate::tokenizer::{Doctype, EmptyTag, Pi, ShortTag};
use crate::{LocalName, Namespace, Prefix};

use crate::tendril::{StrTendril, Tendril};

static XML_URI: &str = "http://www.w3.org/XML/1998/namespace";
static XMLNS_URI: &str = "http://www.w3.org/2000/xmlns/";

type InsResult = Result<(), Cow<'static, str>>;

#[derive(Debug)]
struct NamespaceMapStack(Vec<NamespaceMap>);

impl NamespaceMapStack {
    fn new() -> NamespaceMapStack {
        NamespaceMapStack(vec![NamespaceMap::default()])
    }

    fn push(&mut self, map: NamespaceMap) {
        self.0.push(map);
    }

    fn pop(&mut self) {
        self.0.pop();
    }
}

pub(crate) struct NamespaceMap {
    // Map that maps prefixes to URI.
    //
    // Key denotes namespace prefix, and value denotes
    // URI it maps to.
    //
    // If value of value is None, that means the namespace
    // denoted by key has been undeclared.
    scope: BTreeMap<Option<Prefix>, Option<Namespace>>,
}

impl Debug for NamespaceMap {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "\nNamespaceMap[")?;
        for (key, value) in &self.scope {
            writeln!(f, "   {key:?} : {value:?}")?;
        }
        write!(f, "]")
    }
}

impl NamespaceMap {
    // Returns an empty namespace.
    pub(crate) fn empty() -> NamespaceMap {
        NamespaceMap {
            scope: BTreeMap::new(),
        }
    }

    fn default() -> NamespaceMap {
        NamespaceMap {
            scope: {
                let mut map = BTreeMap::new();
                map.insert(None, None);
                map.insert(Some(namespace_prefix!("xml")), Some(ns!(xml)));
                map.insert(Some(namespace_prefix!("xmlns")), Some(ns!(xmlns)));
                map
            },
        }
    }

    pub(crate) fn get(&self, prefix: &Option<Prefix>) -> Option<&Option<Namespace>> {
        self.scope.get(prefix)
    }

    pub(crate) fn get_scope_iter(&self) -> Iter<'_, Option<Prefix>, Option<Namespace>> {
        self.scope.iter()
    }

    pub(crate) fn insert(&mut self, name: &QualName) {
        let prefix = name.prefix.as_ref().cloned();
        let namespace = Some(Namespace::from(&*name.ns));
        self.scope.insert(prefix, namespace);
    }

    fn insert_ns(&mut self, attr: &Attribute) -> InsResult {
        if &*attr.value == XMLNS_URI {
            return Err(Borrowed("Can't declare XMLNS URI"));
        };

        let opt_uri = if attr.value.is_empty() {
            None
        } else {
            Some(Namespace::from(&*attr.value))
        };

        let result = match (&attr.name.prefix, &*attr.name.local) {
            (&Some(namespace_prefix!("xmlns")), "xml") => {
                if &*attr.value != XML_URI {
                    Err(Borrowed("XML namespace can't be redeclared"))
                } else {
                    Ok(())
                }
            },

            (&Some(namespace_prefix!("xmlns")), "xmlns") => {
                Err(Borrowed("XMLNS namespaces can't be changed"))
            },

            (&Some(namespace_prefix!("xmlns")), _) | (&None, "xmlns") => {
                // We can have two cases of properly defined xmlns
                // First with default namespace e.g.
                //
                //     <a xmlns = "www.uri.org" />
                let ns_prefix = if &*attr.name.local == "xmlns" {
                    None

                // Second is with named namespace e.g.
                //
                //     <a xmlns:a = "www.uri.org" />
                } else {
                    Some(Prefix::from(&*attr.name.local))
                };

                if opt_uri.is_some() && self.scope.contains_key(&ns_prefix) {
                    Err(Borrowed("Namespace already defined"))
                } else {
                    self.scope.insert(ns_prefix, opt_uri);
                    Ok(())
                }
            },

            (_, _) => Err(Borrowed("Invalid namespace declaration.")),
        };
        result
    }
}

/// Tree builder options, with an impl for Default.
#[derive(Copy, Clone, Default)]
pub struct XmlTreeBuilderOpts {}

/// The XML tree builder.
pub struct XmlTreeBuilder<Handle, Sink> {
    /// Configuration options for XmlTreeBuilder
    _opts: XmlTreeBuilderOpts,

    /// Consumer of tree modifications.
    pub sink: Sink,

    /// The document node, which is created by the sink.
    doc_handle: Handle,

    /// Stack of open elements, most recently added at end.
    open_elems: RefCell<Vec<Handle>>,

    /// Current element pointer.
    curr_elem: RefCell<Option<Handle>>,

    /// Stack of namespace identifiers and namespaces.
    namespace_stack: RefCell<NamespaceMapStack>,

    /// Current namespace identifier
    current_namespace: RefCell<NamespaceMap>,

    /// Current tree builder phase.
    phase: Cell<XmlPhase>,
}
impl<Handle, Sink> XmlTreeBuilder<Handle, Sink>
where
    Handle: Clone,
    Sink: TreeSink<Handle = Handle>,
{
    /// Create a new tree builder which sends tree modifications to a particular `TreeSink`.
    ///
    /// The tree builder is also a `TokenSink`.
    pub fn new(sink: Sink, opts: XmlTreeBuilderOpts) -> XmlTreeBuilder<Handle, Sink> {
        let doc_handle = sink.get_document();
        XmlTreeBuilder {
            _opts: opts,
            sink,
            doc_handle,
            open_elems: RefCell::new(vec![]),
            curr_elem: RefCell::new(None),
            namespace_stack: RefCell::new(NamespaceMapStack::new()),
            current_namespace: RefCell::new(NamespaceMap::empty()),
            phase: Cell::new(XmlPhase::Start),
        }
    }

    /// Call the `Tracer`'s `trace_handle` method on every `Handle` in the tree builder's
    /// internal state.  This is intended to support garbage-collected DOMs.
    pub fn trace_handles(&self, tracer: &dyn Tracer<Handle = Handle>) {
        tracer.trace_handle(&self.doc_handle);
        for e in self.open_elems.borrow().iter() {
            tracer.trace_handle(e);
        }
        if let Some(h) = self.curr_elem.borrow().as_ref() {
            tracer.trace_handle(h);
        }
    }

    // Debug helper
    #[cfg(not(for_c))]
    #[allow(dead_code)]
    fn dump_state(&self, label: String) {
        debug!("dump_state on {label}");
        debug!("    open_elems:");
        for node in self.open_elems.borrow().iter() {
            debug!(" {:?}", self.sink.elem_name(node));
        }
        debug!("");
    }

    #[cfg(for_c)]
    fn debug_step(&self, _mode: XmlPhase, _token: &Token) {}

    #[cfg(not(for_c))]
    fn debug_step(&self, mode: XmlPhase, token: &Token) {
        debug!(
            "processing {:?} in insertion mode {:?}",
            format!("{:?}", token),
            mode
        );
    }

    fn declare_ns(&self, attr: &mut Attribute) {
        if let Err(msg) = self.current_namespace.borrow_mut().insert_ns(attr) {
            self.sink.parse_error(msg);
        } else {
            attr.name.ns = ns!(xmlns);
        }
    }

    fn find_uri(&self, prefix: &Option<Prefix>) -> Result<Option<Namespace>, Cow<'static, str>> {
        let mut uri = Err(Borrowed("No appropriate namespace found"));

        let current_namespace = self.current_namespace.borrow();
        for ns in self
            .namespace_stack
            .borrow()
            .0
            .iter()
            .chain(Some(&*current_namespace))
            .rev()
        {
            if let Some(el) = ns.get(prefix) {
                uri = Ok(el.clone());
                break;
            }
        }
        uri
    }

    fn bind_qname(&self, name: &mut QualName) {
        match self.find_uri(&name.prefix) {
            Ok(uri) => {
                let ns_uri = match uri {
                    Some(e) => e,
                    None => ns!(),
                };
                name.ns = ns_uri;
            },
            Err(msg) => {
                self.sink.parse_error(msg);
            },
        }
    }

    // This method takes in name qualified name and binds it to the
    // existing namespace context.
    //
    // Returns false if the attribute is a duplicate, returns true otherwise.
    fn bind_attr_qname(
        &self,
        present_attrs: &mut HashSet<(Namespace, LocalName)>,
        name: &mut QualName,
    ) -> bool {
        // Attributes don't have default namespace
        let mut not_duplicate = true;

        if name.prefix.is_some() {
            self.bind_qname(name);
            not_duplicate = Self::check_duplicate_attr(present_attrs, name);
        }
        not_duplicate
    }

    fn check_duplicate_attr(
        present_attrs: &mut HashSet<(Namespace, LocalName)>,
        name: &QualName,
    ) -> bool {
        let pair = (name.ns.clone(), name.local.clone());

        if present_attrs.contains(&pair) {
            return false;
        }
        present_attrs.insert(pair);
        true
    }

    fn process_namespaces(&self, tag: &mut Tag) {
        // List of already present namespace local name attribute pairs.
        let mut present_attrs: HashSet<(Namespace, LocalName)> = Default::default();

        let mut new_attr = vec![];
        // First we extract all namespace declarations
        for attr in tag.attrs.iter_mut().filter(|attr| {
            attr.name.prefix == Some(namespace_prefix!("xmlns"))
                || attr.name.local == local_name!("xmlns")
        }) {
            self.declare_ns(attr);
        }

        // Then we bind those namespace declarations to attributes
        for attr in tag.attrs.iter_mut().filter(|attr| {
            attr.name.prefix != Some(namespace_prefix!("xmlns"))
                && attr.name.local != local_name!("xmlns")
        }) {
            if self.bind_attr_qname(&mut present_attrs, &mut attr.name) {
                new_attr.push(attr.clone());
            }
        }
        tag.attrs = new_attr;

        // Then we bind the tags namespace.
        self.bind_qname(&mut tag.name);

        // Finally, we dump current namespace if its unneeded.
        let x = mem::replace(
            &mut *self.current_namespace.borrow_mut(),
            NamespaceMap::empty(),
        );

        // Only start tag doesn't dump current namespace. However, <script /> is treated
        // differently than every other empty tag, so it needs to retain the current
        // namespace as well.
        if tag.kind == StartTag || (tag.kind == EmptyTag && tag.name.local == local_name!("script"))
        {
            self.namespace_stack.borrow_mut().push(x);
        }
    }

    fn process_to_completion(
        &self,
        mut token: Token,
    ) -> ProcessResult<<Self as TokenSink>::Handle> {
        // Queue of additional tokens yet to be processed.
        // This stays empty in the common case where we don't split whitespace.
        let mut more_tokens = VecDeque::new();

        loop {
            let phase = self.phase.get();

            #[allow(clippy::unused_unit)]
            match self.step(phase, token) {
                XmlProcessResult::Done => {
                    let Some(popped_token) = more_tokens.pop_front() else {
                        return ProcessResult::Continue;
                    };
                    token = popped_token;
                },
                XmlProcessResult::Reprocess(m, t) => {
                    self.phase.set(m);
                    token = t;
                },
                XmlProcessResult::Script(node) => {
                    assert!(more_tokens.is_empty());
                    return ProcessResult::Script(node);
                },
            }
        }
    }
}

impl<Handle, Sink> TokenSink for XmlTreeBuilder<Handle, Sink>
where
    Handle: Clone,
    Sink: TreeSink<Handle = Handle>,
{
    type Handle = Handle;

    fn process_token(&self, token: tokenizer::Token) -> ProcessResult<Self::Handle> {
        // Handle `ParseError` and `DoctypeToken`; convert everything else to the local `Token` type.
        let token = match token {
            tokenizer::Token::ParseError(e) => {
                self.sink.parse_error(e);
                return ProcessResult::Done;
            },

            tokenizer::Token::Doctype(d) => Token::Doctype(d),
            tokenizer::Token::ProcessingInstruction(instruction) => Token::Pi(instruction),
            tokenizer::Token::Tag(x) => Token::Tag(x),
            tokenizer::Token::Comment(x) => Token::Comment(x),
            tokenizer::Token::NullCharacter => Token::NullCharacter,
            tokenizer::Token::EndOfFile => Token::Eof,
            tokenizer::Token::Characters(x) => Token::Characters(x),
        };

        self.process_to_completion(token)
    }

    fn end(&self) {
        for node in self.open_elems.borrow_mut().drain(..).rev() {
            self.sink.pop(&node);
        }
    }
}

fn current_node<Handle>(open_elems: &[Handle]) -> &Handle {
    open_elems.last().expect("no current element")
}

impl<Handle, Sink> XmlTreeBuilder<Handle, Sink>
where
    Handle: Clone,
    Sink: TreeSink<Handle = Handle>,
{
    fn current_node(&self) -> Ref<'_, Handle> {
        Ref::map(self.open_elems.borrow(), |elems| {
            elems.last().expect("no current element")
        })
    }

    fn insert_appropriately(&self, child: NodeOrText<Handle>) {
        let open_elems = self.open_elems.borrow();
        let target = current_node(&open_elems);
        self.sink.append(target, child);
    }

    fn insert_tag(&self, tag: Tag) -> XmlProcessResult<Handle> {
        let child = create_element(&self.sink, tag.name, tag.attrs);
        self.insert_appropriately(AppendNode(child.clone()));
        self.add_to_open_elems(child)
    }

    fn append_tag(&self, tag: Tag) -> XmlProcessResult<Handle> {
        let child = create_element(&self.sink, tag.name, tag.attrs);
        self.insert_appropriately(AppendNode(child.clone()));
        self.sink.pop(&child);
        XmlProcessResult::Done
    }

    fn append_tag_to_doc(&self, tag: Tag) -> Handle {
        let child = create_element(&self.sink, tag.name, tag.attrs);

        self.sink
            .append(&self.doc_handle, AppendNode(child.clone()));
        child
    }

    fn add_to_open_elems(&self, el: Handle) -> XmlProcessResult<Handle> {
        self.open_elems.borrow_mut().push(el);

        XmlProcessResult::Done
    }

    fn append_comment_to_doc(&self, text: StrTendril) -> XmlProcessResult<Handle> {
        let comment = self.sink.create_comment(text);
        self.sink.append(&self.doc_handle, AppendNode(comment));
        XmlProcessResult::Done
    }

    fn append_comment_to_tag(&self, text: StrTendril) -> XmlProcessResult<Handle> {
        let open_elems = self.open_elems.borrow();
        let target = current_node(&open_elems);
        let comment = self.sink.create_comment(text);
        self.sink.append(target, AppendNode(comment));
        XmlProcessResult::Done
    }

    fn append_doctype_to_doc(&self, doctype: Doctype) -> XmlProcessResult<Handle> {
        fn get_tendril(opt: Option<StrTendril>) -> StrTendril {
            match opt {
                Some(expr) => expr,
                None => Tendril::new(),
            }
        }
        self.sink.append_doctype_to_document(
            get_tendril(doctype.name),
            get_tendril(doctype.public_id),
            get_tendril(doctype.system_id),
        );
        XmlProcessResult::Done
    }

    fn append_pi_to_doc(&self, pi: Pi) -> XmlProcessResult<Handle> {
        let pi = self.sink.create_pi(pi.target, pi.data);
        self.sink.append(&self.doc_handle, AppendNode(pi));
        XmlProcessResult::Done
    }

    fn append_pi_to_tag(&self, pi: Pi) -> XmlProcessResult<Handle> {
        let open_elems = self.open_elems.borrow();
        let target = current_node(&open_elems);
        let pi = self.sink.create_pi(pi.target, pi.data);
        self.sink.append(target, AppendNode(pi));
        XmlProcessResult::Done
    }

    fn append_text(&self, chars: StrTendril) -> XmlProcessResult<Handle> {
        self.insert_appropriately(AppendText(chars));
        XmlProcessResult::Done
    }

    fn tag_in_open_elems(&self, tag: &Tag) -> bool {
        self.open_elems
            .borrow()
            .iter()
            .any(|a| self.sink.elem_name(a).expanded() == tag.name.expanded())
    }

    // Pop elements until an element from the set has been popped.
    fn pop_until<P>(&self, pred: P)
    where
        P: Fn(ExpandedName) -> bool,
    {
        loop {
            if self.current_node_in(&pred) {
                break;
            }
            self.pop();
        }
    }

    fn current_node_in<TagSet>(&self, set: TagSet) -> bool
    where
        TagSet: Fn(ExpandedName) -> bool,
    {
        // FIXME: take namespace into consideration:
        set(self.sink.elem_name(&self.current_node()).expanded())
    }

    fn close_tag(&self, tag: Tag) -> XmlProcessResult<Handle> {
        debug!(
            "Close tag: current_node.name {:?} \n Current tag {:?}",
            self.sink.elem_name(&self.current_node()),
            &tag.name
        );

        if *self.sink.elem_name(&self.current_node()).local_name() != tag.name.local {
            self.sink
                .parse_error(Borrowed("Current node doesn't match tag"));
        }

        let is_closed = self.tag_in_open_elems(&tag);

        if is_closed {
            self.pop_until(|p| p == tag.name.expanded());
            self.pop();
        }

        XmlProcessResult::Done
    }

    fn no_open_elems(&self) -> bool {
        self.open_elems.borrow().is_empty()
    }

    fn pop(&self) -> Handle {
        self.namespace_stack.borrow_mut().pop();
        let node = self
            .open_elems
            .borrow_mut()
            .pop()
            .expect("no current element");
        self.sink.pop(&node);
        node
    }

    fn stop_parsing(&self) -> XmlProcessResult<Handle> {
        warn!("stop_parsing for XML5 not implemented, full speed ahead!");
        XmlProcessResult::Done
    }
}

fn any_not_whitespace(x: &StrTendril) -> bool {
    !x.bytes()
        .all(|b| matches!(b, b'\t' | b'\r' | b'\n' | b'\x0C' | b' '))
}

impl<Handle, Sink> XmlTreeBuilder<Handle, Sink>
where
    Handle: Clone,
    Sink: TreeSink<Handle = Handle>,
{
    fn step(&self, mode: XmlPhase, token: Token) -> XmlProcessResult<<Self as TokenSink>::Handle> {
        self.debug_step(mode, &token);

        match mode {
            XmlPhase::Start => match token {
                Token::Tag(Tag {
                    kind: StartTag,
                    name,
                    attrs,
                }) => {
                    let tag = {
                        let mut tag = Tag {
                            kind: StartTag,
                            name,
                            attrs,
                        };
                        self.process_namespaces(&mut tag);
                        tag
                    };
                    self.phase.set(XmlPhase::Main);
                    let handle = self.append_tag_to_doc(tag);
                    self.add_to_open_elems(handle)
                },
                Token::Tag(Tag {
                    kind: EmptyTag,
                    name,
                    attrs,
                }) => {
                    let tag = {
                        let mut tag = Tag {
                            kind: EmptyTag,
                            name,
                            attrs,
                        };
                        self.process_namespaces(&mut tag);
                        tag
                    };
                    self.phase.set(XmlPhase::End);
                    let handle = self.append_tag_to_doc(tag);
                    self.sink.pop(&handle);
                    XmlProcessResult::Done
                },
                Token::Comment(comment) => self.append_comment_to_doc(comment),
                Token::Pi(pi) => self.append_pi_to_doc(pi),
                Token::Characters(ref chars) if !any_not_whitespace(chars) => {
                    XmlProcessResult::Done
                },
                Token::Eof => {
                    self.sink
                        .parse_error(Borrowed("Unexpected EOF in start phase"));
                    XmlProcessResult::Reprocess(XmlPhase::End, Token::Eof)
                },
                Token::Doctype(d) => {
                    self.append_doctype_to_doc(d);
                    XmlProcessResult::Done
                },
                _ => {
                    self.sink
                        .parse_error(Borrowed("Unexpected element in start phase"));
                    XmlProcessResult::Done
                },
            },
            XmlPhase::Main => match token {
                Token::Characters(chs) => self.append_text(chs),
                Token::Tag(Tag {
                    kind: StartTag,
                    name,
                    attrs,
                }) => {
                    let tag = {
                        let mut tag = Tag {
                            kind: StartTag,
                            name,
                            attrs,
                        };
                        self.process_namespaces(&mut tag);
                        tag
                    };
                    self.insert_tag(tag)
                },
                Token::Tag(Tag {
                    kind: EmptyTag,
                    name,
                    attrs,
                }) => {
                    let tag = {
                        let mut tag = Tag {
                            kind: EmptyTag,
                            name,
                            attrs,
                        };
                        self.process_namespaces(&mut tag);
                        tag
                    };
                    if tag.name.local == local_name!("script") {
                        self.insert_tag(tag.clone());
                        let script = current_node(&self.open_elems.borrow()).clone();
                        self.close_tag(tag);
                        XmlProcessResult::Script(script)
                    } else {
                        self.append_tag(tag)
                    }
                },
                Token::Tag(Tag {
                    kind: EndTag,
                    name,
                    attrs,
                }) => {
                    let tag = {
                        let mut tag = Tag {
                            kind: EndTag,
                            name,
                            attrs,
                        };
                        self.process_namespaces(&mut tag);
                        tag
                    };
                    if tag.name.local == local_name!("script") {
                        let script = current_node(&self.open_elems.borrow()).clone();
                        self.close_tag(tag);
                        if self.no_open_elems() {
                            self.phase.set(XmlPhase::End);
                        }
                        return XmlProcessResult::Script(script);
                    }
                    let retval = self.close_tag(tag);
                    if self.no_open_elems() {
                        self.phase.set(XmlPhase::End);
                    }
                    retval
                },
                Token::Tag(Tag { kind: ShortTag, .. }) => {
                    self.pop();
                    if self.no_open_elems() {
                        self.phase.set(XmlPhase::End);
                    }
                    XmlProcessResult::Done
                },
                Token::Comment(comment) => self.append_comment_to_tag(comment),
                Token::Pi(pi) => self.append_pi_to_tag(pi),
                Token::Eof | Token::NullCharacter => {
                    XmlProcessResult::Reprocess(XmlPhase::End, Token::Eof)
                },
                Token::Doctype(_) => {
                    self.sink
                        .parse_error(Borrowed("Unexpected element in main phase"));
                    XmlProcessResult::Done
                },
            },
            XmlPhase::End => match token {
                Token::Comment(comment) => self.append_comment_to_doc(comment),
                Token::Pi(pi) => self.append_pi_to_doc(pi),
                Token::Characters(ref chars) if !any_not_whitespace(chars) => {
                    XmlProcessResult::Done
                },
                Token::Eof => self.stop_parsing(),
                _ => {
                    self.sink
                        .parse_error(Borrowed("Unexpected element in end phase"));
                    XmlProcessResult::Done
                },
            },
        }
    }
}
