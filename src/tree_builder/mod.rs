/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

// FIXME
#![allow(unused_imports)]

pub use self::interface::{QuirksMode, Quirks, LimitedQuirks, NoQuirks};
pub use self::interface::TreeSink;

use tokenizer;
use tokenizer::{Doctype, Attribute, AttrName, TagKind, StartTag, EndTag, Tag};
use tokenizer::TokenSink;

use util::str::strip_leading_whitespace;
use util::atom::Atom;
use util::namespace::HTML;

use std::default::Default;
use std::mem::replace;

mod interface;
mod states;
mod data;

/// We mostly only work with these tokens. Everything else is handled
/// specially at the beginning of `process_in_mode`.
#[deriving(Eq, TotalEq, Clone, Show)]
enum Token {
    TagToken(Tag),
    CommentToken(StrBuf),
    CharacterTokens(StrBuf),
    EOFToken,
}

/// Tree builder options, with an impl for Default.
#[deriving(Clone)]
pub struct TreeBuilderOpts {
    /// Is scripting enabled?
    pub scripting_enabled: bool,

    /// Is this an iframe srcdoc document?
    pub iframe_srcdoc: bool,
}

impl Default for TreeBuilderOpts {
    fn default() -> TreeBuilderOpts {
        TreeBuilderOpts {
            scripting_enabled: true,
            iframe_srcdoc: false,
        }
    }
}

pub struct TreeBuilder<'sink, Handle, Sink> {
    /// Options controlling the behavior of the tree builder.
    opts: TreeBuilderOpts,

    /// Consumer of tree modifications.
    sink: &'sink mut Sink,

    /// Insertion mode.
    mode: states::InsertionMode,

    /// Original insertion mode, used by Text and InTableText modes.
    orig_mode: states::InsertionMode,

    /// The document node, which is created by the sink.
    doc_handle: Handle,

    /// Stack of open elements, most recently added at end.
    open_elems: Vec<Handle>,

    /// Head element pointer.
    head_elem: Option<Handle>,
}

/// Remove leading whitespace from character tokens;
/// return None if the entire string is removed.
fn drop_whitespace(token: Token) -> Option<Token> {
    match token {
        CharacterTokens(ref x) => match strip_leading_whitespace(x.as_slice()) {
            "" => return None,
            // FIXME: We don't absolutely need to copy here, but it's hard
            // to correctly handle reconsumption without it.
            y if y.len() != x.len() => return Some(CharacterTokens(y.to_strbuf())),
            _ => (),
        },
        _ => (),
    }
    // Fall through to un-borrow `token`.
    Some(token)
}

// We use guards, so we can't bind tags by move.  Instead, bind by ref
// mut and take attrs with `replace`.  This is basically fine since
// empty `Vec` doesn't allocate.
fn take_attrs(t: &mut Tag) -> Vec<Attribute> {
    replace(&mut t.attrs, vec!())
}

enum ProcessResult {
    Done,
    Reprocess(states::InsertionMode, Token),
}

macro_rules! drop_whitespace ( ($x:expr) => (
    unwrap_or_return!(drop_whitespace($x), Done)
))

macro_rules! tag_pattern (
    ($kind:ident     $var:ident) => ( TagToken(ref     $var @ Tag { kind: $kind, ..}) );
    ($kind:ident mut $var:ident) => ( TagToken(ref mut $var @ Tag { kind: $kind, ..}) );
)

macro_rules! start ( ($($args:tt)*) => ( tag_pattern!(StartTag $($args)*) ))
macro_rules! end   ( ($($args:tt)*) => ( tag_pattern!(EndTag   $($args)*) ))

macro_rules! named ( ($t:expr, $($atom:ident)*) => (
    match_atom!($t.name { $($atom)* => true, _ => false })
))

macro_rules! kind_named ( ($kind:ident $t:expr, $($atom:ident)*) => (
    match $t {
        tag_pattern!($kind t) => named!(t, $($atom)*),
        _ => false,
    }
))

macro_rules! start_named ( ($($args:tt)*) => ( kind_named!(StartTag $($args)*) ))
macro_rules! end_named   ( ($($args:tt)*) => ( kind_named!(EndTag   $($args)*) ))

impl<'sink, Handle: Clone, Sink: TreeSink<Handle>> TreeBuilder<'sink, Handle, Sink> {
    pub fn new(sink: &'sink mut Sink, opts: TreeBuilderOpts) -> TreeBuilder<'sink, Handle, Sink> {
        let doc_handle = sink.get_document();
        TreeBuilder {
            opts: opts,
            sink: sink,
            mode: states::Initial,
            orig_mode: states::Initial,
            doc_handle: doc_handle,
            open_elems: vec!(),
            head_elem: None,
        }
    }

    // The "appropriate place for inserting a node".
    fn target(&self) -> Handle {
        // FIXME: foster parenting, templates, other nonsense
        self.open_elems.last().expect("no current element").clone()
    }

    fn push(&mut self, elem: &Handle) {
        self.open_elems.push(elem.clone());
    }

    fn create_root(&mut self, attrs: Vec<Attribute>) {
        let elem = self.sink.create_element(HTML, atom!(html), attrs);
        self.push(&elem);
        self.sink.append_element(self.doc_handle.clone(), elem);
        // FIXME: application cache selection algorithm
    }

    fn create_element(&mut self, name: Atom, attrs: Vec<Attribute>) -> Handle {
        let target = self.target();
        let elem = self.sink.create_element(HTML, name, attrs);
        self.push(&elem);
        self.sink.append_element(target, elem.clone());
        // FIXME: Remove from the stack if we can't append?
        elem
    }

    fn process_in_mode(&mut self, mut mode: states::InsertionMode, token: tokenizer::Token) {
        // Handle `ParseError` and `DoctypeToken`; convert everything else to the local `Token` type.
        let mut token = match token {
            tokenizer::ParseError(e) => {
                self.sink.parse_error(e);
                return;
            }

            tokenizer::DoctypeToken(dt) => if mode == states::Initial {
                let (err, quirk) = data::doctype_error_and_quirks(&dt, self.opts.iframe_srcdoc);
                if err {
                    self.sink.parse_error(format!("Bad DOCTYPE: {}", dt));
                }
                let Doctype { name, public_id, system_id, force_quirks: _ } = dt;
                self.sink.append_doctype_to_document(
                    name.unwrap_or(StrBuf::new()),
                    public_id.unwrap_or(StrBuf::new()),
                    system_id.unwrap_or(StrBuf::new())
                );
                self.sink.set_quirks_mode(quirk);

                self.mode = states::BeforeHtml;
                return;
            } else {
                self.sink.parse_error(format!("DOCTYPE in insertion mode {:?}", mode));
                return;
            },

            tokenizer::TagToken(x) => TagToken(x),
            tokenizer::CommentToken(x) => CommentToken(x),
            tokenizer::CharacterTokens(x) => CharacterTokens(x),
            tokenizer::EOFToken => EOFToken,
        };

        loop {
            match self.process_local(mode, token) {
                Done => return,
                Reprocess(m, t) => {
                    mode = m;
                    token = t;
                }
            }
        }
    }

    fn process_local(&mut self, mode: states::InsertionMode, token: Token) -> ProcessResult {
        debug!("processing {} in insertion mode {:?}", token, mode);

        match mode {
            states::Initial => match drop_whitespace!(token) {
                CommentToken(text) => {
                    self.sink.append_comment(self.doc_handle.clone(), text);
                    Done
                }
                token => {
                    if !self.opts.iframe_srcdoc {
                        self.sink.parse_error(format!("Bad token in Initial insertion mode: {}", token));
                        self.sink.set_quirks_mode(Quirks);
                    }
                    Reprocess(states::BeforeHtml, token)
                }
            },

            states::BeforeHtml => match drop_whitespace!(token) {
                CommentToken(text) => {
                    self.sink.append_comment(self.doc_handle.clone(), text);
                    Done
                }
                start!(mut t) if named!(t, html) => {
                    self.create_root(take_attrs(t));
                    Done
                }
                end!(t) if !named!(t, head body html br) => {
                    self.sink.parse_error(format!("Unexpected end tag in BeforeHtml mode: {}", t));
                    Done
                }
                token => {
                    self.create_root(vec!());
                    Reprocess(states::BeforeHead, token)
                }
            },

            states::BeforeHead => match drop_whitespace!(token) {
                CommentToken(text) => {
                    let target = self.target();
                    self.sink.append_comment(target, text);
                    Done
                }
                end!(t) if !named!(t, head body html br) => {
                    self.sink.parse_error(format!("Unexpected end tag in BeforeHead mode: {}", t));
                    Done
                }
                start!(mut t) if named!(t, head) => {
                    self.head_elem = Some(self.create_element(atom!(head), take_attrs(t)));
                    self.mode = states::InHead;
                    Done
                }
                token => if start_named!(token, html) {
                    // Do this here because we can't move out of `token` when it's borrowed.
                    self.process_local(states::InBody, token)
                } else {
                    self.head_elem = Some(self.create_element(atom!(head), vec!()));
                    Reprocess(states::InHead, token)
                },
            },

              states::InHead
            | states::InHeadNoscript
            | states::AfterHead
            | states::InBody
            | states::Text
            | states::InTable
            | states::InTableText
            | states::InCaption
            | states::InColumnGroup
            | states::InTableBody
            | states::InRow
            | states::InCell
            | states::InSelect
            | states::InSelectInTable
            | states::InTemplate
            | states::AfterBody
            | states::InFrameset
            | states::AfterFrameset
            | states::AfterAfterBody
            | states::AfterAfterFrameset
                => fail!("not implemented"),
        }
    }
}

impl<'sink, Handle: Clone, Sink: TreeSink<Handle>> TokenSink for TreeBuilder<'sink, Handle, Sink> {
    fn process_token(&mut self, token: tokenizer::Token) {
        self.process_in_mode(self.mode, token);
    }
}

test_eq!(drop_not_characters, drop_whitespace(EOFToken), Some(EOFToken))
test_eq!(drop_all_whitespace, drop_whitespace(CharacterTokens("    ".to_strbuf())), None)
test_eq!(drop_some_whitespace,
    drop_whitespace(CharacterTokens("   hello".to_strbuf())),
    Some(CharacterTokens("hello".to_strbuf())))
test_eq!(drop_no_whitespace,
    drop_whitespace(CharacterTokens("hello".to_strbuf())),
    Some(CharacterTokens("hello".to_strbuf())))

#[test]
fn empty_drop_doesnt_reallocate() {
    fn get_ptr(token: &Token) -> *u8 {
        match *token {
            CharacterTokens(ref x) => x.as_slice().as_ptr(),
            _ => fail!("not characters"),
        }
    }

    let x = CharacterTokens("hello".to_strbuf());
    let p = get_ptr(&x);
    assert_eq!(p, get_ptr(&drop_whitespace(x).unwrap()));
}
