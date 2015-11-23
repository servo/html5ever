mod actions;
mod rules;
mod types;
// "pub" is a workaround for rust#18241 (?)
pub mod interface;

use std::collections::{VecDeque, BTreeMap};
use std::result::Result;
use std::borrow::{Cow};
use std::borrow::Cow::Borrowed;

use string_cache::Atom;

use tokenizer::{self, TokenSink, Tag, QName, Attribute};
pub use self::interface::{TreeSink, Tracer, NextParserState, NodeOrText};
use self::rules::XmlTreeBuilderStep;
use self::types::*;

macro_rules! atoms {
    () => (Atom::from(""));
    (xml) => (Atom::from("xml"));
    (xml_uri) => (Atom::from("http://www.w3.org/XML/1998/namespace"));
    (xmlns) => (Atom::from("xmlns"));
    (xmlns_uri) => (Atom::from("http://www.w3.org/2000/xmlns/"))
}

enum InsResult {
    Ok,
    Err(Cow<'static, str>),
}

struct NamespaceStack(Vec<Namespace>);

impl NamespaceStack{
    pub fn new() -> NamespaceStack {
        NamespaceStack({
            let mut vec = Vec::new();
            vec.push(Namespace::default());
            vec
        })
    }

    pub fn push(&mut self, namespace: Namespace) {
        self.0.push(namespace);
    }

    pub fn pop(&mut self) {
        self.0.pop();
    }

    pub fn peek_mut(&mut self) -> Option<&mut Namespace> {
        self.0.last_mut()
    }

    pub fn find_uri(&self, prefix: &Atom) ->  Result<Option<Atom>, Cow<'static, str> > {
        let mut uri = Err(Borrowed("No appropriate namespace found"));
        for ns in self.0.iter().rev() {
            if let Some(el) = ns.get(prefix) {
                uri = Ok(el.clone());
                break;
            }
        }

        uri
    }
}

pub type UriMapping = (Atom, Atom);

struct Namespace {
    // Map that maps prefixes to URI.
    //
    // Key denotes namespace prefix, and value denotes
    // URI it maps to.
    //
    // If value of value is None, that means the namespace
    // denoted by key has been undeclared.
    scope: BTreeMap<Atom, Option<Atom>>,
}

impl Namespace {
    // Returns an empty namespace.
    fn empty() -> Namespace {
        Namespace{
            scope: BTreeMap::new(),
        }
    }

    fn default() -> Namespace {
        Namespace {
            scope: {
                let mut map = BTreeMap::new();
                map.insert(atoms!(), Some(atoms!()));
                map.insert(atoms!(xml), Some(atoms!(xml_uri)));
                map.insert(atoms!(xmlns), Some(atoms!(xmlns_uri)));
                map
            },
        }
    }

    fn is_predefined(&self, ns: &Atom) -> bool {
        *ns == atoms!(xml) || *ns == atoms!(xmlns)
    }

    fn get(&self, prefix: &Atom) -> Option<&Option<Atom>> {
        self.scope.get(prefix)
    }


    fn insert_ns(&mut self, ns: &Atom, uri: &Atom) -> InsResult {
        let result;
        let opt_uri = if uri == &atoms!() {
            None
        } else {
            Some(uri.clone())
        };

        if !self.is_predefined(&ns) {
            if self.scope.contains_key(&ns) && opt_uri.is_some() {
                result = InsResult::Err(Borrowed("Namespace already defined"));
            } else {
                self.scope.insert(ns.clone(), opt_uri);
                result = InsResult::Ok;
            }
        } else {
            if ns == &atoms!(xmlns) {
                result = InsResult::Err(Borrowed("XMLNS namespace can't be altered"));
            } else {
                if opt_uri.is_some() {
                    result = InsResult::Err(Borrowed("XML namespace can only be undeclared"));
                } else {
                    self.scope.insert(atoms!(xml), None);
                    result = InsResult::Ok;
                }
            }
        }
        result
    }
}


// The XML tree builder.
pub struct XmlTreeBuilder<Handle, Sink> {
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
    namespace_stack: NamespaceStack,

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
    pub fn new(mut sink: Sink) -> XmlTreeBuilder<Handle, Sink> {
        let doc_handle = sink.get_document();
        XmlTreeBuilder {
            sink: sink,
            doc_handle: doc_handle,
            next_tokenizer_state: None,
            open_elems: vec!(),
            curr_elem: None,
            namespace_stack: NamespaceStack::new(),
            phase: StartPhase,
        }
    }

    pub fn unwrap(self) -> Sink {
        self.sink
    }

    pub fn sink<'a>(&'a self) -> &'a Sink {
        &self.sink
    }

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

        println!("dump_state on {}", label);
        print!("    open_elems:");
        for node in self.open_elems.iter() {
            let QName { prefix, local, .. } = self.sink.elem_name(node);
            print!(" {:?}:{:?}", prefix,local);

        }
        println!("");
    }

    #[cfg(for_c)]
    fn debug_step(&self, _mode: XmlPhase, _token: &Token) {
    }

    #[cfg(not(for_c))]
    fn debug_step(&self, mode: XmlPhase, token: &Token) {
        debug!("processing {:?} in insertion mode {:?}", format!("{:?}", token), mode);
    }

    fn create_top_namespace(&mut self) {
        self.namespace_stack.push(Namespace::empty());
    }

    fn insert_ns(&mut self, attr: &mut Attribute, uri: Atom) {
        let top_ns = self.namespace_stack.peek_mut();

        if let Some(nsc) = top_ns {
            if let InsResult::Err(msg) =
                    nsc.insert_ns(&attr.name.prefix, &uri) {

                self.sink.parse_error(msg);
            } else {
                attr.name.namespace_url = uri.clone();
            }
        }
    }

    fn bind_qname(&mut self, name: &mut QName) {
        match self.namespace_stack.find_uri(&name.prefix) {
            Ok(uri) => {
                let ns_uri = match uri {
                    Some(e) => e,
                    None => atoms!(),
                };
                // TODO: check duplicates
                name.namespace_url = ns_uri;
            },
            Err(msg) => {
                self.sink.parse_error(msg);
            },
        }
    }

/**

        fn check_dupl(
            set: &mut HashSet<(Atom, Option<Atom>)>,
            tuple: (Atom, Option<Atom>)
        ) -> InsResult {
            let result;
            if set.contains(&tuple) {
                result = InsResult::Err(Borrowed("Repeated namespace, this namespace will be ignored"));
            } else {
                set.insert(tuple);
                result = InsResult::Ok;
            }
            result
        }
*/

    fn process_namespaces(&mut self, tag: &mut Tag) {

        self.create_top_namespace();


        for mut attr in tag.attrs.iter_mut() {

            let is_pseudo_decl = &attr.name.prefix == &atoms!(xmlns)
                                    || attr.name.local == atoms!(xmlns);

            let uri = Atom::from(&*attr.value);

            if is_pseudo_decl {
                self.insert_ns(&mut attr, uri);
            } else {
                self.bind_qname(&mut attr.name);
            }

        }

        self.bind_qname(&mut tag.name);


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
        //let ignore_lf = replace(&mut self.ignore_lf, false);

        // Handle `ParseError` and `DoctypeToken`; convert everything else to the local `Token` type.
        let token = match token {
            tokenizer::ParseError(e) => {
                self.sink.parse_error(e);
                return;
            }

            tokenizer::DoctypeToken(_) => {
                panic!("Doctype not implemented!!");
            }

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
