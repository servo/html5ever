// Copyright 2015 The xml5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod actions;
mod rules;
mod types;
// "pub" is a workaround for rust#18241 (?)
pub mod interface;

use std::borrow::{Cow};
use std::borrow::Cow::Borrowed;
use std::collections::{VecDeque, BTreeMap, HashSet};
use std::collections::btree_map::{Iter};
use std::result::Result;
use std::mem;

use {Prefix, Namespace, LocalName};

use tokenizer::{self, TokenSink, Tag, QName, Attribute, StartTag};
pub use self::interface::{TreeSink, Tracer, NextParserState, NodeOrText};
use self::rules::XmlTreeBuilderStep;
use self::types::*;

static XML_URI: &'static str = "http://www.w3.org/XML/1998/namespace";
static XMLNS_URI: &'static str = "http://www.w3.org/2000/xmlns/";

type InsResult = Result<(), Cow<'static, str>>;

#[derive(Debug)]
struct NamespaceMapStack(Vec<NamespaceMap>);


impl NamespaceMapStack{
    fn new() -> NamespaceMapStack {
        NamespaceMapStack({
            let mut vec = Vec::new();
            vec.push(NamespaceMap::default());
            vec
        })
    }

    fn push(&mut self, map: NamespaceMap) {
        self.0.push(map);
    }

    #[doc(hidden)]
    pub fn pop(&mut self) {
        self.0.pop();
    }

}

#[derive(Debug)]
struct NamespaceMap {
    // Map that maps prefixes to URI.
    //
    // Key denotes namespace prefix, and value denotes
    // URI it maps to.
    //
    // If value of value is None, that means the namespace
    // denoted by key has been undeclared.
    scope: BTreeMap<Prefix, Option<Namespace>>,
}


impl NamespaceMap {
    // Returns an empty namespace.
    fn empty() -> NamespaceMap {
        NamespaceMap {
            scope: BTreeMap::new(),
        }
    }

    fn default() -> NamespaceMap {
            scope: {
                let mut map = BTreeMap::new();
                map.insert(namespace_prefix!(""), None);
                map.insert(namespace_prefix!("xml"), Some(ns!(xml)));
                map.insert(namespace_prefix!("xmlns"), Some(ns!(xmlns)));
                map
            },
        }
    }
}


    fn get(&self, prefix: &Prefix) -> Option<&Option<Namespace>> {
        self.scope.get(prefix)
    }

    #[doc(hidden)]
    pub fn get_scope_iter(&self) -> Iter<Atom, Option<Atom>> {
        self.scope.iter()
    }

    #[doc(hidden)]
    pub fn insert(&mut self, name: &QName)  {
        let prefix = Atom::from(&*name.prefix);
        let namespace = Some(Atom::from(&*name.namespace_url));
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

        let result = match (&*attr.name.prefix, &*attr.name.local) {
            ("xmlns", "xml") => {
                if &*attr.value != XML_URI {
                    Err(Borrowed("XML namespace can't be redeclared"))
                } else {
                    Ok(())
                }
            },

            ("xmlns" , "xmlns") => {
                Err(Borrowed("XMLNS namespaces can't be changed"))
            },

            ("xmlns", _)
            | ("", "xmlns")=> {
                let ext = if attr.name.prefix.is_empty() {
                    namespace_prefix!("")
                } else {
                    Prefix::from(&*attr.name.local)
                };

                if self.scope.contains_key(&ext) && opt_uri.is_some() {
                    Err(Borrowed("Namespace already defined"))
                } else {
                    self.scope.insert(ext, opt_uri);
                    Ok(())
                }
            },

            (_, _) => {
                Err(Borrowed("Invalid namespace declaration."))
            }
        };
        result
    }
}
/// Tree builder options, with an impl for Default.
#[derive(Copy, Clone)]
pub struct XmlTreeBuilderOpts {
    /// Report all parse errors described in the spec, at some
    /// performance penalty?  Default: false
    pub exact_errors: bool,

    /// Keep a record of how long we spent in each state?  Printed
    /// when `end()` is called.  Default: false
    pub profile: bool,
}

impl Default for XmlTreeBuilderOpts {
    fn default() -> XmlTreeBuilderOpts {
        XmlTreeBuilderOpts{
            exact_errors: false,
            profile: false,
        }
    }
}

/// The XML tree builder.
pub struct XmlTreeBuilder<Handle, Sink> {
    /// Configuration options for XmlTreeBuilder
    opts: XmlTreeBuilderOpts,

    /// Consumer of tree modifications.
    sink: Sink,

    /// The document node, which is created by the sink.
    doc_handle: Handle,

    /// Next state change for the tokenizer, if any.
    next_tokenizer_state: Option<tokenizer::states::XmlState>,

    /// Stack of open elements, most recently added at end.
    open_elems: Vec<Handle>,

    /// Current element pointer.
    curr_elem: Option<Handle>,

    /// Stack of namespace identifiers and namespaces.
    namespace_stack: NamespaceMapStack,

    /// Current namespace identifier
    current_namespace: NamespaceMap,

    /// List of already present namespace local name attribute pairs.
    present_attrs: HashSet<(Namespace, LocalName)>,

    /// Current tree builder phase.
    phase: XmlPhase,
}
impl<Handle, Sink> XmlTreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{
    /// Create a new tree builder which sends tree modifications to a particular `TreeSink`.
    ///
    /// The tree builder is also a `TokenSink`.
    pub fn new(mut sink: Sink, opts: XmlTreeBuilderOpts) -> XmlTreeBuilder<Handle, Sink> {
        let doc_handle = sink.get_document();
        XmlTreeBuilder {
            opts: opts,
            sink: sink,
            doc_handle: doc_handle,
            next_tokenizer_state: None,
            open_elems: vec!(),
            curr_elem: None,
            namespace_stack: NamespaceMapStack::new(),
            current_namespace: NamespaceMap::empty(),
            present_attrs: HashSet::new(),
            phase: StartPhase,
        }
    }

    /// Returns consumer of tree modifications.
    pub fn unwrap(self) -> Sink {
        self.sink
    }

    /// Immutably borrows consumer of tree modifications.
    pub fn sink<'a>(&'a self) -> &'a Sink {
        &self.sink
    }

    /// Mutably borrows consumer of tree modifications.
    pub fn sink_mut<'a>(&'a mut self) -> &'a mut Sink {
        &mut self.sink
    }

    /// Call the `Tracer`'s `trace_handle` method on every `Handle` in the tree builder's
    /// internal state.  This is intended to support garbage-collected DOMs.
    pub fn trace_handles(&self, tracer: &Tracer<Handle=Handle>) {
        tracer.trace_handle(self.doc_handle.clone());
        for e in self.open_elems.iter() {
            tracer.trace_handle(e.clone());
        }
        self.curr_elem.as_ref().map(|h| tracer.trace_handle(h.clone()));
    }

    // Debug helper
    #[cfg(not(for_c))]
    #[allow(dead_code)]
    fn dump_state(&self, label: String) {

        debug!("dump_state on {}", label);
        debug!("    open_elems:");
        for node in self.open_elems.iter() {
            let QName { prefix, local, .. } = self.sink.elem_name(node);
            debug!(" {:?}:{:?}", prefix,local);

        }
        debug!("");
    }

    #[cfg(for_c)]
    fn debug_step(&self, _mode: XmlPhase, _token: &Token) {
    }

    #[cfg(not(for_c))]
    fn debug_step(&self, mode: XmlPhase, token: &Token) {
        debug!("processing {:?} in insertion mode {:?}", format!("{:?}", token), mode);
    }


    fn declare_ns(&mut self, attr: &mut Attribute) {
        if let Err(msg) = self.current_namespace.insert_ns(&attr) {
            self.sink.parse_error(msg);

        } else {
            attr.name.namespace_url = ns!(xmlns);
        }

    }

    fn find_uri(&self, prefix: &Prefix) ->  Result<Option<Namespace>, Cow<'static, str> >{
        let mut uri = Err(Borrowed("No appropriate namespace found"));
        for ns in self.namespace_stack.0.iter()
                    .chain(Some(&self.current_namespace)).rev() {
            if let Some(el) = ns.get(prefix) {
                uri = Ok(el.clone());
                break;
            }
        }
        uri
    }

    fn bind_qname(&mut self, name: &mut QName) {
        match self.find_uri(&name.prefix) {
            Ok(uri) => {
                let ns_uri = match uri {
                    Some(e) => e,
                    None => ns!(),
                };
                name.namespace_url = ns_uri;
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
    fn bind_attr_qname(&mut self, name: &mut QName) -> bool {
        // Attributes don't have default namespace
        let mut not_duplicate = true;

        if !name.prefix.is_empty() {
            self.bind_qname(name);
            not_duplicate = self.check_duplicate_attr(name);
        }
        not_duplicate
    }

    fn check_duplicate_attr(&mut self, name: &QName) -> bool {
        let pair = (name.namespace_url.clone(), name.local.clone());

        if self.present_attrs.contains(&pair) {
            return false;
        }
        self.present_attrs.insert(pair);
        true
    }

    fn process_namespaces(&mut self, tag: &mut Tag) {
        let mut new_attr = vec![];
        // First we extract all namespace declarations
        for mut attr in tag.attrs.iter_mut()
            .filter(|attr| attr.name.prefix == namespace_prefix!("xmlns")
                          || attr.name.local == local_name!("xmlns")) {

            self.declare_ns(&mut attr);
        }

        // Then we bind those namespace declarations to attributes
        for mut attr in tag.attrs.iter_mut()
            .filter(|attr| attr.name.prefix != namespace_prefix!("xmlns")
                          && attr.name.local != local_name!("xmlns")) {

            if self.bind_attr_qname(&mut attr.name) {
                new_attr.push(attr.clone());
            }

        }
        mem::replace(&mut tag.attrs, new_attr);
        // Then we bind the tags namespace.
        self.bind_qname(&mut tag.name);

        // Finally, we dump current namespace if its unneeded.
        let x = mem::replace(&mut self.current_namespace, NamespaceMap::empty());

        // Only start tag doesn't dump current namespace.
        if tag.kind == StartTag {
            self.namespace_stack.push(x);
        }

    }

    fn process_to_completion(&mut self, mut token: Token) {
        // Queue of additional tokens yet to be processed.
        // This stays empty in the common case where we don't split whitespace.
        let mut more_tokens = VecDeque::new();

        loop {
            let phase = self.phase;
            match self.step(phase, token) {
                Done => {
                    token = unwrap_or_return!(more_tokens.pop_front(), ());
                }
                Reprocess(m, t) => {
                    self.phase = m;
                    token = t;
                }

            }
        }
    }
}

impl<Handle, Sink> TokenSink
    for XmlTreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{
    fn process_token(&mut self, token: tokenizer::Token) {

        // Handle `ParseError` and `DoctypeToken`; convert everything else to the local `Token` type.
        let token = match token {
            tokenizer::ParseError(e) => {
                self.sink.parse_error(e);
                return;
            }

            tokenizer::DoctypeToken(d) =>DoctypeToken(d),
            tokenizer::PIToken(x)   => PIToken(x),
            tokenizer::TagToken(x) => TagToken(x),
            tokenizer::CommentToken(x) => CommentToken(x),
            tokenizer::NullCharacterToken => NullCharacterToken,
            tokenizer::EOFToken => EOFToken,
            tokenizer::CharacterTokens(x) => CharacterTokens(x),

        };

        self.process_to_completion(token);
    }

    fn query_state_change(&mut self) -> Option<tokenizer::states::XmlState> {
        self.next_tokenizer_state.take()
    }
}
