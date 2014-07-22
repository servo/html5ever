// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The HTML5 tree builder.

pub use self::interface::{QuirksMode, Quirks, LimitedQuirks, NoQuirks};
pub use self::interface::TreeSink;

use tokenizer;
use tokenizer::{Doctype, Attribute, Tag, StartTag};
use tokenizer::TokenSink;
use tokenizer::states::{RawData, RawKind, Rcdata, Rawtext, ScriptData, Plaintext};

use util::atom::Atom;
use util::namespace::{Namespace, HTML};
use util::str::{is_ascii_whitespace, Runs, to_escaped_string};

use std::default::Default;
use std::mem::replace;
use std::ascii::StrAsciiExt;
use std::iter::{Rev, Enumerate};
use std::slice;

mod interface;
mod data;

#[deriving(PartialEq, Eq, Clone, Show)]
pub enum InsertionMode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    InHeadNoscript,
    AfterHead,
    InBody,
    Text,
    InTable,
    InTableText,
    InCaption,
    InColumnGroup,
    InTableBody,
    InRow,
    InCell,
    InSelect,
    InSelectInTable,
    InTemplate,
    AfterBody,
    InFrameset,
    AfterFrameset,
    AfterAfterBody,
    AfterAfterFrameset,
}

#[deriving(PartialEq, Eq, Clone, Show)]
enum SplitStatus {
    NotSplit,
    Whitespace,
    NotWhitespace,
}

/// We mostly only work with these tokens. Everything else is handled
/// specially at the beginning of `process_token`.
#[deriving(PartialEq, Eq, Clone, Show)]
enum Token {
    TagToken(Tag),
    CommentToken(String),
    CharacterTokens(SplitStatus, String),
    NullCharacterToken,
    EOFToken,
}

/// Tree builder options, with an impl for Default.
#[deriving(Clone)]
pub struct TreeBuilderOpts {
    /// Is scripting enabled?
    pub scripting_enabled: bool,

    /// Is this an `iframe srcdoc` document?
    pub iframe_srcdoc: bool,

    /// Are we parsing a HTML fragment?
    pub fragment: bool,
}

impl Default for TreeBuilderOpts {
    fn default() -> TreeBuilderOpts {
        TreeBuilderOpts {
            scripting_enabled: true,
            iframe_srcdoc: false,
            fragment: false,
        }
    }
}

enum FormatEntry<Handle> {
    Element(Handle, Tag),
    Marker,
}

struct ActiveFormattingIter<'a, Handle> {
    iter: Rev<Enumerate<slice::Items<'a, FormatEntry<Handle>>>>,
}

impl<'a, Handle> Iterator<(uint, &'a Handle, &'a Tag)> for ActiveFormattingIter<'a, Handle> {
    fn next(&mut self) -> Option<(uint, &'a Handle, &'a Tag)> {
        match self.iter.next() {
            None | Some((_, &Marker)) => None,
            Some((i, &Element(ref h, ref t))) => Some((i, h, t)),
        }
    }
}

/// The HTML tree builder.
pub struct TreeBuilder<'sink, Handle, Sink> {
    /// Options controlling the behavior of the tree builder.
    opts: TreeBuilderOpts,

    /// Consumer of tree modifications.
    sink: &'sink mut Sink,

    /// Insertion mode.
    mode: InsertionMode,

    /// Original insertion mode, used by Text and InTableText modes.
    orig_mode: Option<InsertionMode>,

    /// Quirks mode as set by the parser.
    /// FIXME: can scripts etc. change this?
    quirks_mode: QuirksMode,

    /// The document node, which is created by the sink.
    doc_handle: Handle,

    /// Stack of open elements, most recently added at end.
    open_elems: Vec<Handle>,

    /// List of active formatting elements.
    active_formatting: Vec<FormatEntry<Handle>>,

    /// Head element pointer.
    head_elem: Option<Handle>,

    /// Form element pointer.
    form_elem: Option<Handle>,

    /// Next state change for the tokenizer, if any.
    next_tokenizer_state: Option<tokenizer::states::State>,

    /// Frameset-ok flag.
    frameset_ok: bool,

    /// Ignore a following U+000A LINE FEED?
    ignore_lf: bool,
}

enum ProcessResult {
    Done,
    DoneAckSelfClosing,
    SplitWhitespace(String),
    Reprocess(InsertionMode, Token),
}

enum PushFlag {
    Push,
    NoPush,
}

macro_rules! tag_op_to_bool (
    (+) => (true);
    (-) => (false);
)

macro_rules! declare_tag_set ( ($name:ident = $supr:ident $op:tt $($tag:ident)+) => (
    fn $name(p: (Namespace, Atom)) -> bool {
        match p {
            $( (HTML, atom!($tag)) => tag_op_to_bool!($op), )+
            p => $supr(p),
        }
    }
))

type TagSet<'a> = |(Namespace, Atom)|: 'a -> bool;

#[inline(always)] fn empty_set(_: (Namespace, Atom)) -> bool { false }
#[inline(always)] fn full_set(_: (Namespace, Atom)) -> bool { true }

// FIXME: MathML, SVG
declare_tag_set!(default_scope = empty_set
    + applet caption html table td th marquee object template)

declare_tag_set!(list_item_scope = default_scope + ol ul)
declare_tag_set!(button_scope = default_scope + button)
declare_tag_set!(table_scope = empty_set + html table template)
declare_tag_set!(select_scope = full_set - optgroup option)

declare_tag_set!(cursory_implied_end = empty_set
    + dd dt li option optgroup p rp rt)

declare_tag_set!(thorough_implied_end = cursory_implied_end
    + caption colgroup tbody td tfoot th thead tr)

declare_tag_set!(heading_tag = empty_set + h1 h2 h3 h4 h5 h6)

declare_tag_set!(special_tag = empty_set +
    address applet area article aside base basefont bgsound blockquote body br button caption
    center col colgroup dd details dir div dl dt embed fieldset figcaption figure footer form
    frame frameset h1 h2 h3 h4 h5 h6 head header hgroup hr html iframe img input isindex li
    link listing main marquee menu menuitem meta nav noembed noframes noscript object ol p
    param plaintext pre script section select source style summary table tbody td template
    textarea tfoot th thead title tr track ul wbr xmp)

#[allow(dead_code)]
fn unused_tag_sets() {
    // FIXME: Some tag sets are unused until we implement <template> or other stuff.
    // Suppress the warning here.
    select_scope((HTML, atom!(p)));
    table_scope((HTML, atom!(p)));
    thorough_implied_end((HTML, atom!(p)));
}

macro_rules! append_with ( ( $fun:ident, $target:expr, $($args:expr),* ) => ({
    // two steps to avoid double borrow
    let target = $target;
    self.sink.$fun(target, $($args),*);
    Done
}))

macro_rules! append_text    ( ($target:expr, $text:expr) => ( append_with!(append_text,    $target, $text) ))
macro_rules! append_comment ( ($target:expr, $text:expr) => ( append_with!(append_comment, $target, $text) ))

impl<'sink, Handle: Clone, Sink: TreeSink<Handle>> TreeBuilder<'sink, Handle, Sink> {
    /// Create a new tree builder which sends tree modifications to a particular `TreeSink`.
    ///
    /// The tree builder is also a `TokenSink`.
    pub fn new(sink: &'sink mut Sink, opts: TreeBuilderOpts) -> TreeBuilder<'sink, Handle, Sink> {
        let doc_handle = sink.get_document();
        TreeBuilder {
            opts: opts,
            sink: sink,
            mode: Initial,
            orig_mode: None,
            quirks_mode: NoQuirks,
            doc_handle: doc_handle,
            open_elems: vec!(),
            active_formatting: vec!(),
            head_elem: None,
            form_elem: None,
            next_tokenizer_state: None,
            frameset_ok: true,
            ignore_lf: false,
        }
    }

    /// Iterate over the active formatting elements (with index in the list) from the end
    /// to the last marker, or the beginning if there are no markers.
    fn active_formatting_end_to_marker<'a>(&'a self) -> ActiveFormattingIter<'a, Handle> {
        ActiveFormattingIter {
            iter: self.active_formatting.iter().enumerate().rev(),
        }
    }

    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        self.quirks_mode = mode;
        self.sink.set_quirks_mode(mode);
    }

    fn stop_parsing(&mut self) -> ProcessResult {
        error!("stop_parsing not implemented, full speed ahead!");
        Done
    }

    // Switch to `Text` insertion mode, save the old mode, and
    // switch the tokenizer to a raw-data state.
    // The latter only takes effect after the current / next
    // `process_token` of a start tag returns!
    fn to_raw_text_mode(&mut self, k: RawKind) {
        assert!(self.next_tokenizer_state.is_none());
        self.next_tokenizer_state = Some(RawData(k));
        self.orig_mode = Some(self.mode);
        self.mode = Text;
    }

    // The generic raw text / RCDATA parsing algorithm.
    fn parse_raw_data(&mut self, tag: Tag, k: RawKind) {
        self.insert_element_for(tag);
        self.to_raw_text_mode(k);
    }

    fn current_node(&self) -> Handle {
        self.open_elems.last().expect("no current element").clone()
    }

    fn current_node_in(&self, set: TagSet) -> bool {
        set(self.sink.elem_name(self.current_node()))
    }

    // The "appropriate place for inserting a node".
    fn target(&self) -> Handle {
        // FIXME: foster parenting, templates, other nonsense
        self.current_node()
    }

    fn adoption_agency(&mut self, subject: Atom) {
        // FIXME: this is not right
        if self.current_node_named(subject) {
            self.pop();
        }
    }

    fn push(&mut self, elem: &Handle) {
        self.open_elems.push(elem.clone());
    }

    fn pop(&mut self) -> Handle {
        self.open_elems.pop().expect("no current element")
    }

    fn remove_from_stack(&mut self, elem: &Handle) {
        let mut open_elems = replace(&mut self.open_elems, vec!());
        open_elems.retain(|x| !self.sink.same_node(elem.clone(), x.clone()));
        self.open_elems = open_elems;
    }

    /// Reconstruct the active formatting elements.
    fn reconstruct_formatting(&mut self) {
        // FIXME
    }

    /// Get the first element on the stack, which will be the <html> element.
    fn html_elem(&self) -> Handle {
         self.open_elems.get(0).clone()
    }

    /// Get the second element on the stack, if it's a HTML body element.
    fn body_elem(&mut self) -> Option<Handle> {
        if self.open_elems.len() <= 1 {
            return None;
        }

        let node = self.open_elems.get(1).clone();
        if self.html_elem_named(node.clone(), atom!(body)) {
            Some(node)
        } else {
            None
        }
    }

    /// Signal an error depending on the state of the stack of open elements at
    /// the end of the body.
    fn check_body_end(&mut self) {
        declare_tag_set!(body_end_ok = empty_set
            + dd dt li optgroup option p rp rt tbody td tfoot th
              thead tr body html)

        for elem in self.open_elems.iter() {
            let name = self.sink.elem_name(elem.clone());
            if !body_end_ok(name.clone()) {
                self.sink.parse_error(
                    format!("Unexpected open tag {} at end of body", name));
                // FIXME: Do we keep checking after finding one bad tag?
                // The spec suggests not.
                return;
            }
        }
    }

    fn in_scope(&self, scope: TagSet, pred: |Handle| -> bool) -> bool {
        for node in self.open_elems.iter().rev() {
            if pred(node.clone()) {
                return true;
            }
            if scope(self.sink.elem_name(node.clone())) {
                return false;
            }
        }

        // supposed to be impossible, because <html> is always in scope

        false
    }

    fn elem_in(&self, elem: Handle, set: TagSet) -> bool {
        set(self.sink.elem_name(elem))
    }

    fn html_elem_named(&self, elem: Handle, name: Atom) -> bool {
        self.sink.elem_name(elem) == (HTML, name)
    }

    fn current_node_named(&self, name: Atom) -> bool {
        self.html_elem_named(self.current_node(), name)
    }

    fn in_scope_named(&self, scope: TagSet, name: Atom) -> bool {
        self.in_scope(scope, |elem|
            self.html_elem_named(elem, name.clone()))
    }

    fn generate_implied_end(&mut self, set: TagSet) {
        loop {
            let elem = unwrap_or_return!(self.open_elems.last(), ()).clone();
            let nsname = self.sink.elem_name(elem);
            if !set(nsname) { return; }
            self.pop();
        }
    }

    fn generate_implied_end_except(&mut self, except: Atom) {
        self.generate_implied_end(|p| match p {
            (HTML, ref name) if *name == except => true,
            _ => cursory_implied_end(p),
        });
    }

    // Pop elements until an element from the set has been popped.  Returns the
    // number of elements popped.
    fn pop_until(&mut self, pred: TagSet) -> uint {
        let mut n = 0;
        loop {
            match self.open_elems.pop() {
                None => break,
                Some(elem) => if pred(self.sink.elem_name(elem)) { break; },
            }
            n += 1;
        }
        n
    }

    // Pop elements until one with the specified name has been popped.
    // Signal an error if it was not the first one.
    fn expect_to_close(&mut self, name: Atom) {
        if self.pop_until(|p| p == (HTML, name.clone())) != 1 {
            self.sink.parse_error(
                format!("Unexpected open element while closing {}", name));
        }
    }

    fn close_p_element(&mut self) {
        declare_tag_set!(implied = cursory_implied_end - p);
        self.generate_implied_end(implied);
        self.expect_to_close(atom!(p));
    }

    fn close_p_element_in_button_scope(&mut self) {
        if self.in_scope_named(button_scope, atom!(p)) {
            self.close_p_element();
        }
    }

    fn create_root(&mut self, attrs: Vec<Attribute>) {
        let elem = self.sink.create_element(HTML, atom!(html), attrs);
        self.push(&elem);
        self.sink.append_element(self.doc_handle.clone(), elem);
        // FIXME: application cache selection algorithm
    }

    fn insert_element(&mut self, push: PushFlag, name: Atom, attrs: Vec<Attribute>)
            -> Handle {
        let target = self.target();
        let elem = self.sink.create_element(HTML, name, attrs);
        match push {
            Push => self.push(&elem),
            NoPush => (),
        }
        self.sink.append_element(target, elem.clone());
        // FIXME: Remove from the stack if we can't append?
        elem
    }

    fn insert_element_for(&mut self, tag: Tag) -> Handle {
        self.insert_element(Push, tag.name, tag.attrs)
    }

    fn insert_and_pop_element_for(&mut self, tag: Tag) -> Handle {
        self.insert_element(NoPush, tag.name, tag.attrs)
    }

    fn create_formatting_element_for(&mut self, tag: Tag) -> Handle {
        // FIXME: This really wants unit tests.
        let mut first_match = None;
        let mut matches = 0u;
        for (i, _, old_tag) in self.active_formatting_end_to_marker() {
            if tag.equiv_modulo_attr_order(old_tag) {
                first_match = Some(i);
                matches += 1;
            }
        }

        if matches >= 3 {
            self.active_formatting.remove(first_match.expect("matches with no index"));
        }

        let elem = self.insert_element(Push, tag.name.clone(), tag.attrs.clone());
        self.active_formatting.push(Element(elem.clone(), tag));
        elem
    }

    fn clear_active_formatting_to_marker(&mut self) {
        loop {
            match self.active_formatting.pop() {
                None | Some(Marker) => break,
                _ => (),
            }
        }
    }

    fn process_to_completion(&mut self, mut token: Token) {
        // Additional tokens yet to be processed. First to be processed is on
        // the *end*, because that's where Vec supports O(1) push/pop.
        // This stays empty (and hence non-allocating) in the common case
        // where we don't split whitespace.
        let mut more_tokens = vec!();

        loop {
            let is_self_closing = match token {
                TagToken(Tag { self_closing: c, .. }) => c,
                _ => false,
            };
            let mode = self.mode;
            match self.step(mode, token) {
                Done => {
                    if is_self_closing {
                        self.sink.parse_error("Unacknowledged self-closing tag".to_string());
                    }
                    token = unwrap_or_return!(more_tokens.pop(), ());
                }
                DoneAckSelfClosing => {
                    token = unwrap_or_return!(more_tokens.pop(), ());
                }
                Reprocess(m, t) => {
                    self.mode = m;
                    token = t;
                }
                SplitWhitespace(buf) => {
                    let mut it = Runs::new(is_ascii_whitespace, buf.as_slice())
                        .map(|(m, b)| CharacterTokens(match m {
                            true => Whitespace,
                            false => NotWhitespace,
                        }, b.to_string()));

                    token = unwrap_or_return!(it.next(), ());

                    // Push additional tokens in reverse order, so the next one
                    // is first to be popped.
                    // FIXME: copy/allocate less
                    let rest: Vec<Token> = it.collect();
                    for t in rest.move_iter().rev() {
                        more_tokens.push(t);
                    }
                }
            }
        }
    }

    fn step(&mut self, mode: InsertionMode, token: Token) -> ProcessResult {
        // $thing may be either a Token or a Tag
        macro_rules! unexpected ( ($thing:expr) => ({
            self.sink.parse_error(format!("Unexpected token {} in insertion mode {}",
                to_escaped_string(&$thing), mode));
            Done
        }))

        debug!("processing {} in insertion mode {:?}", to_escaped_string(&token), mode);

        match mode {
            Initial => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => Done,
                CommentToken(text) => append_comment!(self.doc_handle.clone(), text),
                token => {
                    if !self.opts.iframe_srcdoc {
                        unexpected!(token);
                        self.set_quirks_mode(Quirks);
                    }
                    Reprocess(BeforeHtml, token)
                }
            }),

            BeforeHtml => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => Done,
                CommentToken(text) => append_comment!(self.doc_handle.clone(), text),

                tag @ <html> => {
                    self.create_root(tag.attrs);
                    self.mode = BeforeHead;
                    Done
                }

                </head> </body> </html> </br> => else,

                tag @ </_> => unexpected!(tag),

                token => {
                    self.create_root(vec!());
                    Reprocess(BeforeHead, token)
                }
            }),

            BeforeHead => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => Done,
                CommentToken(text) => append_comment!(self.target(), text),

                <html> => self.step(InBody, token),

                tag @ <head> => {
                    self.head_elem = Some(self.insert_element_for(tag));
                    self.mode = InHead;
                    Done
                }

                </head> </body> </html> </br> => else,

                tag @ </_> => unexpected!(tag),

                token => {
                    self.head_elem = Some(self.insert_element(Push, atom!(head), vec!()));
                    Reprocess(InHead, token)
                }
            }),

            InHead => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => append_text!(self.target(), text),
                CommentToken(text) => append_comment!(self.target(), text),

                <html> => self.step(InBody, token),

                tag @ <base> <basefont> <bgsound> <link> <meta> => {
                    // FIXME: handle <meta charset=...> and <meta http-equiv="Content-Type">
                    self.insert_and_pop_element_for(tag);
                    DoneAckSelfClosing
                }

                tag @ <title> => {
                    self.parse_raw_data(tag, Rcdata);
                    Done
                }

                tag @ <noframes> <style> <noscript> => {
                    if (!self.opts.scripting_enabled) && (tag.name == atom!(noscript)) {
                        self.insert_element_for(tag);
                        self.mode = InHeadNoscript;
                    } else {
                        self.parse_raw_data(tag, Rawtext);
                    }
                    Done
                }

                tag @ <script> => {
                    let target = self.target();
                    let elem = self.sink.create_element(HTML, atom!(script), tag.attrs);
                    if self.opts.fragment {
                        self.sink.mark_script_already_started(elem.clone());
                    }
                    self.push(&elem);
                    self.sink.append_element(target, elem);
                    self.to_raw_text_mode(ScriptData);
                    Done
                }

                </head> => {
                    self.pop();
                    self.mode = AfterHead;
                    Done
                }

                </body> </html> </br> => else,

                <template> => fail!("FIXME: <template> not implemented"),
                </template> => fail!("FIXME: <template> not implemented"),

                <head> => unexpected!(token),
                tag @ </_> => unexpected!(tag),

                token => {
                    self.pop();
                    Reprocess(AfterHead, token)
                }
            }),

            InHeadNoscript => match_token!(token {
                <html> => self.step(InBody, token),

                </noscript> => {
                    self.pop();
                    self.mode = InHead;
                    Done
                },

                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InHead, token),

                CommentToken(_) => self.step(InHead, token),

                <basefont> <bgsound> <link> <meta> <noframes> <style>
                    => self.step(InHead, token),

                </br> => else,

                <head> <noscript> => unexpected!(token),
                tag @ </_> => unexpected!(tag),

                token => {
                    unexpected!(token);
                    self.pop();
                    Reprocess(InHead, token)
                },
            }),

            AfterHead => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => append_text!(self.target(), text),
                CommentToken(text) => append_comment!(self.target(), text),

                <html> => self.step(InBody, token),

                tag @ <body> => {
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    self.mode = InBody;
                    Done
                }

                tag @ <frameset> => {
                    self.insert_element_for(tag);
                    self.mode = InFrameset;
                    Done
                }

                <base> <basefont> <bgsound> <link> <meta>
                      <noframes> <script> <style> <template> <title> => {
                    unexpected!(token);
                    let head = self.head_elem.as_ref().expect("no head element").clone();
                    self.push(&head);
                    let result = self.step(InHead, token);
                    self.remove_from_stack(&head);
                    result
                }

                </template> => self.step(InHead, token),

                </body> </html> </br> => else,

                <head> => unexpected!(token),
                tag @ </_> => unexpected!(tag),

                token => {
                    self.insert_element(Push, atom!(body), vec!());
                    Reprocess(InBody, token)
                }
            }),

            InBody => match_token!(token {
                NullCharacterToken => unexpected!(token),

                CharacterTokens(_, text) => {
                    self.reconstruct_formatting();
                    // FIXME: this might be much faster as a byte scan
                    let unset_frameset_ok = text.as_slice().chars().any(|c| !is_ascii_whitespace(c));
                    append_text!(self.target(), text);
                    if unset_frameset_ok {
                        self.frameset_ok = false;
                    }
                    Done
                }

                CommentToken(text) => append_comment!(self.target(), text),

                tag @ <html> => {
                    unexpected!(tag);
                    // FIXME: <template>
                    let top = self.html_elem();
                    self.sink.add_attrs_if_missing(top, tag.attrs);
                    Done
                }

                <base> <basefont> <bgsound> <link> <meta> <noframes>
                  <script> <style> <template> <title> </template> => {
                    self.step(InHead, token)
                }

                tag @ <body> => {
                    unexpected!(tag);
                    // FIXME: <template>
                    match self.body_elem() {
                        None => (),
                        Some(node) => {
                            self.frameset_ok = false;
                            self.sink.add_attrs_if_missing(node, tag.attrs)
                        }
                    }
                    Done
                }

                tag @ <frameset> => {
                    unexpected!(tag);
                    if !self.frameset_ok { return Done; }

                    let body = unwrap_or_return!(self.body_elem(), Done);
                    self.sink.remove_from_parent(body);

                    // FIXME: can we get here in the fragment case?
                    // What to do with the first element then?
                    self.open_elems.truncate(1);
                    self.insert_element_for(tag);
                    self.mode = InFrameset;
                    Done
                }

                EOFToken => {
                    // FIXME: <template>
                    self.check_body_end();
                    self.stop_parsing()
                }

                </body> => {
                    if self.in_scope_named(default_scope, atom!(body)) {
                        self.check_body_end();
                        self.mode = AfterBody;
                    } else {
                        self.sink.parse_error("</body> with no <body> in scope".to_string());
                    }
                    Done
                }

                </html> => {
                    if self.in_scope_named(default_scope, atom!(body)) {
                        self.check_body_end();
                        Reprocess(AfterBody, token)
                    } else {
                        self.sink.parse_error("</html> with no <body> in scope".to_string());
                        Done
                    }
                }

                tag @ <address> <article> <aside> <blockquote> <center> <details> <dialog>
                  <dir> <div> <dl> <fieldset> <figcaption> <figure> <footer> <header>
                  <hgroup> <main> <menu> <nav> <ol> <p> <section> <summary> <ul> => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    Done
                }

                tag @ <h1> <h2> <h3> <h4> <h5> <h6> => {
                    self.close_p_element_in_button_scope();
                    if self.current_node_in(heading_tag) {
                        self.sink.parse_error("nested heading tags".to_string());
                        self.pop();
                    }
                    self.insert_element_for(tag);
                    Done
                }

                tag @ <pre> <listing> => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    self.ignore_lf = true;
                    self.frameset_ok = false;
                    Done
                }

                tag @ <form> => {
                    // FIXME: <template>
                    if self.form_elem.is_some() {
                        self.sink.parse_error("nested forms".to_string());
                    } else {
                        self.close_p_element_in_button_scope();
                        let elem = self.insert_element_for(tag);
                        // FIXME: <template>
                        self.form_elem = Some(elem);
                    }
                    Done
                }

                <li> => fail!("FIXME: <li> not implemented"),
                <dd> <dt> => fail!("FIXME: <dd> <dt> not implemented"),

                tag @ <plaintext> => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    self.next_tokenizer_state = Some(Plaintext);
                    Done
                }

                tag @ <button> => {
                    if self.in_scope_named(default_scope, atom!(button)) {
                        self.sink.parse_error("nested buttons".to_string());
                        self.generate_implied_end(cursory_implied_end);
                        self.pop_until(|p| p == (HTML, atom!(button)));
                    }
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    Done
                }

                tag @ </address> </article> </aside> </blockquote> </button> </center>
                  </details> </dialog> </dir> </div> </dl> </fieldset> </figcaption>
                  </figure> </footer> </header> </hgroup> </listing> </main> </menu>
                  </nav> </ol> </pre> </section> </summary> </ul> => {
                    if !self.in_scope_named(default_scope, tag.name.clone()) {
                        unexpected!(tag);
                    } else {
                        self.generate_implied_end(cursory_implied_end);
                        self.expect_to_close(tag.name);
                    }
                    Done
                }

                </form> => {
                    // FIXME: <template>
                    let node = unwrap_or_return!(self.form_elem.take(), {
                        self.sink.parse_error("Null form element pointer on </form>".to_string());
                        Done
                    });
                    if !self.in_scope(default_scope,
                        |n| self.sink.same_node(node.clone(), n)) {
                        self.sink.parse_error("Form element not in scope on </form>".to_string());
                        return Done;
                    }
                    self.generate_implied_end(cursory_implied_end);
                    let current = self.current_node();
                    self.remove_from_stack(&node);
                    if !self.sink.same_node(current, node) {
                        self.sink.parse_error("Bad open element on </form>".to_string());
                    }
                    Done
                }

                </p> => {
                    if !self.in_scope_named(button_scope, atom!(p)) {
                        self.sink.parse_error("No <p> tag to close".to_string());
                        self.insert_element(Push, atom!(p), vec!());
                    }
                    self.close_p_element();
                    Done
                }

                tag @ </li> </dd> </dt> => {
                    let scope = match tag.name {
                        atom!(li) => list_item_scope,
                        _ => default_scope,
                    };
                    if self.in_scope_named(|x| scope(x), tag.name.clone()) {
                        self.generate_implied_end_except(tag.name.clone());
                        self.expect_to_close(tag.name);
                    } else {
                        self.sink.parse_error(format!("No {} tag to close", tag.name));
                    }
                    Done
                }

                tag @ </h1> </h2> </h3> </h4> </h5> </h6> => {
                    if self.in_scope(default_scope, |n| self.elem_in(n.clone(), heading_tag)) {
                        self.generate_implied_end(cursory_implied_end);
                        if !self.current_node_named(tag.name) {
                            self.sink.parse_error("Closing wrong heading tag".to_string());
                        }
                        self.pop_until(heading_tag);
                    } else {
                        self.sink.parse_error("No heading tag to close".to_string());
                    }
                    Done
                }

                tag @ <a> => {
                    let mut to_remove = vec!();
                    for (i, handle, _) in self.active_formatting_end_to_marker() {
                        if self.html_elem_named(handle.clone(), atom!(a)) {
                            to_remove.push((i, handle.clone()));
                        }
                    }

                    if !to_remove.is_empty() {
                        unexpected!(tag);
                        self.adoption_agency(atom!(a));
                        // FIXME: quadratic time
                        for (i, handle) in to_remove.move_iter() {
                            self.remove_from_stack(&handle);
                            self.active_formatting.remove(i);
                            // We iterated backwards from the end above, so
                            // we don't need to adjust the indices after each
                            // removal.
                        }
                    }

                    self.reconstruct_formatting();
                    self.create_formatting_element_for(tag);
                    Done
                }

                tag @ <b> <big> <code> <em> <font> <i> <s> <small> <strike> <strong> <tt> <u> => {
                    self.reconstruct_formatting();
                    self.create_formatting_element_for(tag);
                    Done
                }

                tag @ <nobr> => {
                    self.reconstruct_formatting();
                    if self.in_scope_named(default_scope, atom!(nobr)) {
                        self.sink.parse_error("Nested <nobr>".to_string());
                        self.adoption_agency(atom!(nobr));
                        self.reconstruct_formatting();
                    }
                    self.create_formatting_element_for(tag);
                    Done
                }

                tag @ </a> </b> </big> </code> </em> </font> </i> </nobr>
                  </s> </small> </strike> </strong> </tt> </u> => {
                    self.adoption_agency(tag.name);
                    Done
                }

                tag @ <applet> <marquee> <object> => {
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    self.active_formatting.push(Marker);
                    self.frameset_ok = false;
                    Done
                }

                tag @ </applet> </marquee> </object> => {
                    if !self.in_scope_named(default_scope, tag.name.clone()) {
                        unexpected!(tag);
                    } else {
                        self.generate_implied_end(cursory_implied_end);
                        self.expect_to_close(tag.name);
                        self.clear_active_formatting_to_marker();
                    }
                    Done
                }

                tag @ <table> => {
                    if self.quirks_mode != Quirks {
                        self.close_p_element_in_button_scope();
                    }
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    self.mode = InTable;
                    Done
                }

                tag @ </br> => {
                    unexpected!(tag);
                    self.step(InBody, TagToken(Tag {
                        kind: StartTag,
                        attrs: vec!(),
                        ..tag
                    }))
                }

                tag @ <area> <br> <embed> <img> <keygen> <wbr> <input> => {
                    let keep_frameset_ok = match tag.name {
                        atom!(input) => {
                            match tag.attrs.iter().find(|&at| at.name.name == atom!("type")) {
                                None => false,
                                Some(at) => at.value.as_slice().eq_ignore_ascii_case("hidden"),
                            }
                        }
                        _ => false,
                    };
                    self.reconstruct_formatting();
                    self.insert_and_pop_element_for(tag);
                    if !keep_frameset_ok {
                        self.frameset_ok = false;
                    }
                    DoneAckSelfClosing
                }

                tag @ <menuitem> <param> <source> <track> => {
                    self.insert_and_pop_element_for(tag);
                    DoneAckSelfClosing
                }

                tag @ <hr> => {
                    self.close_p_element_in_button_scope();
                    self.insert_and_pop_element_for(tag);
                    self.frameset_ok = false;
                    DoneAckSelfClosing
                }

                tag @ <image> => {
                    unexpected!(tag);
                    self.step(InBody, TagToken(Tag {
                        name: atom!(img),
                        ..tag
                    }))
                }

                <isindex> => fail!("FIXME: <isindex> not implemented"),

                tag @ <textarea> => {
                    self.ignore_lf = true;
                    self.frameset_ok = false;
                    self.parse_raw_data(tag, Rcdata);
                    Done
                }

                tag @ <xmp> => {
                    self.close_p_element_in_button_scope();
                    self.reconstruct_formatting();
                    self.frameset_ok = false;
                    self.parse_raw_data(tag, Rawtext);
                    Done
                }

                tag @ <iframe> => {
                    self.frameset_ok = false;
                    self.parse_raw_data(tag, Rawtext);
                    Done
                }

                tag @ <noembed> => {
                    self.parse_raw_data(tag, Rawtext);
                    Done
                }

                // <noscript> handled in wildcard case below

                tag @ <select> => {
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    // NB: mode == InBody but possibly self.mode != mode, if
                    // we're processing "as in the rules for InBody".
                    self.mode = match self.mode {
                        InTable | InCaption | InTableBody
                            | InRow | InCell => InSelectInTable,
                        _ => InSelect,
                    };
                    Done
                }

                tag @ <optgroup> <option> => {
                    if self.current_node_named(atom!(option)) {
                        self.pop();
                    }
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    Done
                }

                tag @ <rp> <rt> => {
                    if self.in_scope_named(default_scope, atom!(ruby)) {
                        self.generate_implied_end(cursory_implied_end);
                    }
                    if !self.current_node_named(atom!(ruby)) {
                        unexpected!(tag);
                    }
                    self.insert_element_for(tag);
                    Done
                }

                <math> => fail!("FIXME: MathML not implemented"),
                <svg> => fail!("FIXME: SVG not implemented"),

                <caption> <col> <colgroup> <frame> <head>
                  <tbody> <td> <tfoot> <th> <thead> <tr> => {
                    unexpected!(token);
                    Done
                }

                tag @ <_> => {
                    if self.opts.scripting_enabled && tag.name == atom!(noscript) {
                        self.parse_raw_data(tag, Rawtext);
                    } else {
                        self.reconstruct_formatting();
                        self.insert_element_for(tag);
                    }
                    Done
                }

                tag @ </_> => {
                    // Look back for a matching open element.
                    let mut match_idx = None;
                    for (i, elem) in self.open_elems.iter().enumerate().rev() {
                        if self.html_elem_named(elem.clone(), tag.name.clone()) {
                            match_idx = Some(i);
                            break;
                        }

                        if self.elem_in(elem.clone(), special_tag) {
                            unexpected!(tag);
                            return Done;
                        }
                    }

                    let match_idx = unwrap_or_return!(match_idx, {
                        // I believe this is impossible, because the root
                        // <html> element is in special_tag.
                        unexpected!(tag);
                        Done
                    });

                    self.generate_implied_end(|p| match p {
                        (HTML, ref name) if *name == tag.name => false,
                        _ => cursory_implied_end(p),
                    });

                    if match_idx != self.open_elems.len() - 1 {
                        // mis-nested tags
                        unexpected!(tag);
                    }
                    self.open_elems.truncate(match_idx);
                    Done
                }

                // FIXME: This should be unreachable, but match_token! requires a
                // catch-all case.
                _ => fail!("impossible case in InBody mode"),
            }),

            Text => match_token!(token {
                CharacterTokens(_, text) => append_text!(self.target(), text),

                EOFToken => {
                    unexpected!(token);
                    if self.current_node_named(atom!(script)) {
                        let current = self.current_node();
                        self.sink.mark_script_already_started(current);
                    }
                    self.pop();
                    Reprocess(self.orig_mode.take_unwrap(), token)
                }

                </script> => fail!("FIXME: </script> not implemented (!)"),

                </_> => {
                    self.pop();
                    self.mode = self.orig_mode.take_unwrap();
                    Done
                }

                // The spec doesn't say what to do here.
                // Other tokens are impossible?
                _ => fail!("impossible case in Text mode"),
            }),

              InTable
            | InTableText
            | InCaption
            | InColumnGroup
            | InTableBody
            | InRow
            | InCell
            | InSelect
            | InSelectInTable
                => fail!("FIXME: table mode {} not implemented", mode),

            InTemplate
                => fail!("FIXME: <template> not implemented"),

            AfterBody => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InBody, token),
                CommentToken(text) => append_comment!(self.html_elem(), text),

                <html> => self.step(InBody, token),

                </html> => {
                    if self.opts.fragment {
                        unexpected!(token);
                    } else {
                        self.mode = AfterAfterBody;
                    }
                    Done
                }

                EOFToken => self.stop_parsing(),

                token => {
                    unexpected!(token);
                    Reprocess(InBody, token)
                }
            }),

            InFrameset => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => append_text!(self.target(), text),
                CommentToken(text) => append_comment!(self.target(), text),

                <html> => self.step(InBody, token),

                tag @ <frameset> => {
                    self.insert_element_for(tag);
                    Done
                }

                </frameset> => {
                    if self.open_elems.len() == 1 {
                        unexpected!(token);
                    } else {
                        self.pop();
                        if !self.opts.fragment && !self.current_node_named(atom!(frameset)) {
                            self.mode = AfterFrameset;
                        }
                    }
                    Done
                }

                tag @ <frame> => {
                    self.insert_and_pop_element_for(tag);
                    DoneAckSelfClosing
                }

                <noframes> => self.step(InHead, token),

                EOFToken => {
                    if self.open_elems.len() != 1 {
                        unexpected!(token);
                    }
                    self.stop_parsing()
                }

                token => unexpected!(token),
            }),

            AfterFrameset => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => append_text!(self.target(), text),
                CommentToken(text) => append_comment!(self.target(), text),

                <html> => self.step(InBody, token),

                </html> => {
                    self.mode = AfterAfterFrameset;
                    Done
                }

                <noframes> => self.step(InHead, token),

                EOFToken => self.stop_parsing(),

                token => unexpected!(token),
            }),

            AfterAfterBody => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InBody, token),
                CommentToken(text) => append_comment!(self.doc_handle.clone(), text),

                <html> => self.step(InBody, token),

                EOFToken => self.stop_parsing(),

                token => {
                    unexpected!(token);
                    Reprocess(InBody, token)
                }
            }),

            AfterAfterFrameset => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InBody, token),
                CommentToken(text) => append_comment!(self.doc_handle.clone(), text),

                <html> => self.step(InBody, token),

                EOFToken => self.stop_parsing(),

                <noframes> => self.step(InHead, token),

                token => unexpected!(token),
            }),
        }
    }
}

impl<'sink, Handle: Clone, Sink: TreeSink<Handle>> TokenSink for TreeBuilder<'sink, Handle, Sink> {
    fn process_token(&mut self, token: tokenizer::Token) {
        let ignore_lf = replace(&mut self.ignore_lf, false);

        // Handle `ParseError` and `DoctypeToken`; convert everything else to the local `Token` type.
        let token = match token {
            tokenizer::ParseError(e) => {
                self.sink.parse_error(e);
                return;
            }

            tokenizer::DoctypeToken(dt) => if self.mode == Initial {
                let (err, quirk) = data::doctype_error_and_quirks(&dt, self.opts.iframe_srcdoc);
                if err {
                    self.sink.parse_error(format!("Bad DOCTYPE: {}", dt));
                }
                let Doctype { name, public_id, system_id, force_quirks: _ } = dt;
                self.sink.append_doctype_to_document(
                    name.unwrap_or(String::new()),
                    public_id.unwrap_or(String::new()),
                    system_id.unwrap_or(String::new())
                );
                self.set_quirks_mode(quirk);

                self.mode = BeforeHtml;
                return;
            } else {
                self.sink.parse_error(format!("DOCTYPE in insertion mode {:?}", self.mode));
                return;
            },

            tokenizer::TagToken(x) => TagToken(x),
            tokenizer::CommentToken(x) => CommentToken(x),
            tokenizer::NullCharacterToken => NullCharacterToken,
            tokenizer::EOFToken => EOFToken,

            tokenizer::CharacterTokens(mut x) => {
                if ignore_lf && x.len() >= 1 && x.as_slice().char_at(0) == '\n' {
                    x.shift_char();
                }
                if x.is_empty() {
                    return;
                }
                CharacterTokens(NotSplit, x)
            }
        };

        self.process_to_completion(token);
    }

    fn query_state_change(&mut self) -> Option<tokenizer::states::State> {
        self.next_tokenizer_state.take()
    }
}
