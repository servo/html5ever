// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The HTML5 tokenizer.

use core::prelude::*;

pub use util::span::{Buf, Span, ValidatedSpanUtils};

pub use self::interface::{Doctype, Attribute, TagKind, StartTag, EndTag, Tag};
pub use self::interface::{Token, DoctypeToken, TagToken, CommentToken};
pub use self::interface::{CharacterTokens, NullCharacterToken, EOFToken, ParseError};
pub use self::interface::TokenSink;

use self::states::{RawLessThanSign, RawEndTagOpen, RawEndTagName};
use self::states::{Rcdata, Rawtext, ScriptData, ScriptDataEscaped};
use self::states::{Escaped, DoubleEscaped};
use self::states::{Unquoted, SingleQuoted, DoubleQuoted};
use self::states::{DoctypeIdKind, Public, System};
use self::states::{ScriptEscapeKind, RawKind};

use self::char_ref::CharRefTokenizer;

use self::buffer_queue::BufferQueue;

use util::fast_option::{Uninit, Full, FastOption, OptValue};
use util::single_char::{SingleChar, MayAppendSingleChar};

use util::str::{is_alphabetic, lower_ascii};
use util::smallcharset::SmallCharSet;

use core::mem::replace;
use core::default::Default;
use alloc::boxed::Box;
use collections::vec::Vec;
#[cfg(not(for_c))]
use collections::slice::SliceAllocPrelude;
use collections::string::String;
use collections::str::{MaybeOwned, Slice};
use collections::TreeMap;

use iobuf::{BufSpan, Iobuf, ROIobuf};

use string_cache::{Atom, QualName};

pub mod states;
mod interface;
mod char_ref;
mod buffer_queue;

fn option_push(opt_str: &mut Option<Span>, c: SingleChar) {
    match *opt_str {
        Some(ref mut s) => s.push_sc(c),
        None => *opt_str = Some(c.into_span()),
    }
}

fn is_errorful_char(c: u32) -> bool {
    match c {
        0x01...0x08 | 0x0B | 0x0E...0x1F | 0x7F...0x9F | 0xFDD0...0xFDEF => true,
        n if (n & 0xFFFE) == 0xFFFE => true,
        _ => false,
    }
}

/// Tokenizer options, with an impl for `Default`.
#[deriving(Clone)]
pub struct TokenizerOpts {
    /// Report all parse errors described in the spec, at some
    /// performance penalty?  Default: false
    pub exact_errors: bool,

    /// Discard a `U+FEFF BYTE ORDER MARK` if we see one at the beginning
    /// of the stream?  Default: true
    pub discard_bom: bool,

    /// Keep a record of how long we spent in each state?  Printed
    /// when `end()` is called.  Default: false
    pub profile: bool,

    /// Initial state override.  Only the test runner should use
    /// a non-`None` value!
    pub initial_state: Option<states::State>,

    /// Last start tag.  Only the test runner should use a
    /// non-`None` value!
    pub last_start_tag_name: Option<String>,
}

impl Default for TokenizerOpts {
    fn default() -> TokenizerOpts {
        TokenizerOpts {
            exact_errors: false,
            discard_bom: true,
            profile: false,
            initial_state: None,
            last_start_tag_name: None,
        }
    }
}

/// Shared state in the tokenizer that the step function needs, but we don't mutate
/// except via get_char and pop_except_from.
struct Shared {
    c: FastOption<SingleChar>,
    r: FastOption<Buf>,
}

pub struct Tokenizer<Sink> {
    shared: Shared,
    inner:  TokenizerInner<Sink>,
}

impl<Sink: TokenSink> Tokenizer<Sink> {
    pub fn new(sink: Sink, opts: TokenizerOpts) -> Tokenizer<Sink> {
        Tokenizer {
            shared: Shared {
                c: FastOption::new(),
                r: FastOption::new(),
            },
            inner: TokenizerInner::new(sink, opts),
        }
    }

    pub fn unwrap(self) -> Sink {
        self.inner.sink
    }

    pub fn sink<'a>(&'a self) -> &'a Sink {
        &self.inner.sink
    }

    pub fn sink_mut<'a>(&'a mut self) -> &'a mut Sink {
        &mut self.inner.sink
    }

    /// Feed an input string into the tokenizer.
    pub fn feed(&mut self, input: Buf) {
        self.inner.feed(input, &mut self.shared)
    }

    pub fn end(&mut self) {
        self.inner.end(&mut self.shared)
    }
}


/// The HTML tokenizer.
struct TokenizerInner<Sink> {
    /// Options controlling the behavior of the tokenizer.
    opts: TokenizerOpts,

    /// Discard a U+FEFF BYTE ORDER MARK if we see one?  Only done at the
    /// beginning of the stream.
    discard_bom: bool,

    /// Destination for tokens we emit.
    sink: Sink,

    /// The abstract machine state as described in the spec.
    state: states::State,

    /// Tokenizer for character references, if we're tokenizing
    /// one at the moment.
    char_ref_tokenizer: Option<Box<CharRefTokenizer>>,

    /// Input ready to be tokenized.
    input_buffers: BufferQueue,

    /// Current input character.  Just consumed, may reconsume.
    current_char: Option<SingleChar>,

    /// Current tag kind.
    current_tag_kind: TagKind,

    /// Current tag name.
    current_tag_name: String,

    /// Current tag is self-closing?
    current_tag_self_closing: bool,

    /// Current tag attributes.
    current_tag_attrs: Vec<Attribute>,

    /// Current attribute name.
    current_attr_name: String,

    /// Current attribute value.
    current_attr_value: Span,

    /// Current comment.
    current_comment: Span,

    /// The buffer representing the first '-' that's ending a comment.
    first_comment_end_dash:  Option<Buf>,
    /// The buffer representing the second '-' that's ending a comment.
    second_comment_end_dash: Option<Buf>,

    /// Another "temporary buffer" not mentioned in the spec. This is used for
    /// when we drop into states that we might soon drop out of, and lets us
    /// keep around the characters that caused the state transitions.
    another_temp_buf: Span,

    /// The "temporary buffer" mentioned in the spec.
    temp_buf: Span,

    /// Current doctype token.
    current_doctype: Doctype,

    /// Current doctype name. This will need to be atomized before loading into
    //// the `current_doctype`.
    current_doctype_name: Option<Span>,

    /// Last start tag name, for use in checking "appropriate end tag".
    last_start_tag_name: Option<Atom>,

    /// Are we at the end of the file, once buffers have been processed
    /// completely? This affects whether we will wait for lookahead or not.
    at_eof: bool,

    /// Record of how many ns we spent in each state, if profiling is enabled.
    state_profile: TreeMap<states::State, u64>,

    /// Record of how many ns we spent in the token sink.
    time_in_sink: u64,

    /// Did we just consume \r, translating it to \n?  In that case we need
    /// to ignore the next character if it's \n.
    ignore_lf: bool,

    /// A cached copy of a single newline character, to deal with CRLF replacement.
    newline: SingleChar,
}

impl<Sink: TokenSink> TokenizerInner<Sink> {
    /// Create a new tokenizer which feeds tokens to a particular `TokenSink`.
    fn new(sink: Sink, mut opts: TokenizerOpts) -> TokenizerInner<Sink> {
        if opts.profile && cfg!(for_c) {
            panic!("Can't profile tokenizer when built as a C library");
        }

        let start_tag_name = opts.last_start_tag_name.take()
            .map(|s| Atom::from_slice(s.as_slice()));
        let state = *opts.initial_state.as_ref().unwrap_or(&states::Data);
        let discard_bom = opts.discard_bom;
        TokenizerInner {
            opts: opts,
            sink: sink,
            state: state,
            char_ref_tokenizer: None,
            input_buffers: BufferQueue::new(),
            at_eof: false,
            current_char: None,
            ignore_lf: false,
            discard_bom: discard_bom,
            current_tag_kind: StartTag,
            current_tag_name: String::new(),
            current_tag_self_closing: false,
            current_tag_attrs: vec!(),
            current_attr_name: String::new(),
            current_attr_value: BufSpan::new(),
            current_comment: BufSpan::new(),
            current_doctype: Doctype::new(),
            current_doctype_name: None,
            last_start_tag_name: start_tag_name,
            temp_buf: BufSpan::new(),
            another_temp_buf: BufSpan::new(),
            first_comment_end_dash: None,
            second_comment_end_dash: None,
            state_profile: TreeMap::new(),
            time_in_sink: 0,
            newline: SingleChar::new(ROIobuf::from_str("\n")),
        }
    }

    /// Feed an input string into the tokenizer.
    fn feed(&mut self, mut input: Buf, shared: &mut Shared) {
        if input.len() == 0 {
            return;
        }

        if self.discard_bom {
            let mut first_three_bytes = [0u8, ..3];
            static UTF8_BOM: [u8, ..3] = [ 0xEF, 0xBB, 0xBF ];

            unsafe {
                match input.peek(0, &mut first_three_bytes) {
                    Err(()) => {},
                    Ok (()) => {
                        if first_three_bytes.as_slice() == UTF8_BOM.as_slice() {
                            input.unsafe_advance(first_three_bytes.len() as u32);
                        }
                    }
                }
            }
        }

        self.input_buffers.push_back(input);
        self.run(shared);
    }

    #[inline(never)]
    fn process_token(&mut self, token: Token) {
        if self.opts.profile {
            self.process_token_slow(token);
        } else {
            self.sink.process_token(token);
        }
    }

    #[inline(never)]
    fn process_token_slow(&mut self, token: Token) {
        let (_, dt) = time!(self.sink.process_token(token));
        self.time_in_sink += dt;
    }

    /// This function is unsafe because the input option `c` MUST be `some` when
    /// you call this function.
    #[inline]
    fn get_preprocessed_char(&mut self, c: &mut FastOption<SingleChar>) -> OptValue {
        if !self.ignore_lf && c.as_ref().as_u8() != b'\r' && !self.opts.exact_errors {
            Full
        } else {
            self.get_preprocessed_char_slow(c)
        }
    }

    #[inline]
    fn get_preprocessed_char_simple(&mut self, c: char) -> Option<char> {
        if !self.ignore_lf && c != '\r' && !self.opts.exact_errors {
            Some(c)
        } else {
            self.get_preprocessed_char_simple_slow(c)
        }
    }

    #[inline(never)]
    fn get_preprocessed_char_simple_slow(&mut self, mut c: char) -> Option<char> {
        if self.ignore_lf {
            self.ignore_lf = false;
            if c == '\n' {
                c = match self.input_buffers.next_simple() {
                    None => return None,
                    Some(c) => c,
                };
            }
        }

        if c == '\r' {
            self.ignore_lf = true;
            c = '\n';
        }

        if self.opts.exact_errors && is_errorful_char(c as u32) {
            // format_if!(true) will still use the static error when built for C.
            let msg = format_if!(true, "Bad character",
                "Bad character {}", c);
            self.emit_error(msg);
        }

        h5e_debug!("got character {}", c);
        Some(c)
    }

    //§ preprocessing-the-input-stream
    // Get the next input character, which might be the character
    // 'c' that we already consumed from the buffers.
    //
    // `c` must be a previously filled FastOption<SingleChar>
    #[inline(never)]
    fn get_preprocessed_char_slow(&mut self, c: &mut FastOption<SingleChar>) -> OptValue {
        if self.ignore_lf {
            self.ignore_lf = false;
            if c.as_ref().as_u8() == b'\n' {
                match self.input_buffers.next(c) {
                    Full   => {},
                    Uninit => return Uninit,
                }
            }
        }

        if c.as_ref().as_u8() == b'\r' {
            self.ignore_lf = true;
            c.fill(self.newline.clone());
        }

        let c_char = c.as_ref().decode_as_char();

        // TODO: This should be possible without actually decoding the char.
        if self.opts.exact_errors && is_errorful_char(c_char as u32) {
            // format_if!(true) will still use the static error when built for C.
            let msg = format_if!(true, "Bad character",
                "Bad character {}", c_char);
            self.emit_error(msg);
        }

        h5e_debug!("got character {}", c_char);
        Full
    }

    //§ tokenization
    // Get the next input character, if one is available.
    #[inline(always)]
    fn get_char(&mut self, dst: &mut FastOption<SingleChar>) -> OptValue {
        if self.current_char.is_none() {
            match self.input_buffers.next(dst) {
                Full   => self.get_preprocessed_char(dst),
                Uninit => Uninit,
            }
        } else {
            self.get_char_slow(dst)
        }
    }

    #[inline(never)]
    fn get_char_slow(&mut self, dst: &mut FastOption<SingleChar>) -> OptValue {
        dst.fill(self.current_char.take().unwrap())
    }

    #[inline(always)]
    fn get_char_simple(&mut self) -> Option<char> {
        if self.current_char.is_none() {
            self.input_buffers.next_simple().and_then(|c| self.get_preprocessed_char_simple(c))
        } else {
            self.get_char_simple_slow()
        }
    }

    #[inline(never)]
    fn get_char_simple_slow(&mut self) -> Option<char> {
        Some(self.current_char.take().unwrap().decode_as_char())
    }

    /// If neither of the `FastOption`s are full, then we're at end of input.
    /// This function will never fill both. Either one or the other. If a char
    /// got popped, `char_dst` will be filled. If a run got popped, `run_dst` will
    /// be filled. The left hand side of the tuple refers to char_dst, and the right
    /// hand side refers to `run_dst`.
    #[inline(always)]
    fn pop_except_from(&mut self, set: SmallCharSet, char_dst: &mut FastOption<SingleChar>, run_dst: &mut FastOption<Buf>) -> (OptValue, OptValue) {
        // Bail to the slow path for various corner cases.
        // This means that `FromSet` can contain characters not in the set!
        // It shouldn't matter because the fallback `FromSet` case should
        // always do the same thing as the `NotFromSet` case.
        if self.opts.exact_errors || self.current_char.is_some() || self.ignore_lf {
            return (self.get_char(char_dst), Uninit);
        }

        let ret = self.input_buffers.pop_except_from(set, char_dst, run_dst);

        match ret {
            (Full, Uninit) => {
                self.get_preprocessed_char(char_dst);
            }
            _ => {},
        };

        ret
    }

    // Check if the next characters are an ASCII case-insensitive match.  See
    // BufferQueue::eat.
    //
    // NB: this doesn't do input stream preprocessing or set the current input
    // character.
    fn eat(&mut self, pat: &[u8]) -> Option<Span> {
        match self.input_buffers.eat(pat) {
            None if self.at_eof => Some(BufSpan::new()),
            r => r,
        }
    }

    // Run the state machine for as long as we can.
    fn run(&mut self, shared: &mut Shared) {
        if self.opts.profile {
            loop {
                let state = self.state;
                let old_sink = self.time_in_sink;
                let (run, mut dt) = time!(self.step(shared));
                dt -= (self.time_in_sink - old_sink);
                let new = match self.state_profile.get_mut(&state) {
                    Some(x) => {
                        *x += dt;
                        false
                    }
                    None => true,
                };
                if new {
                    // do this here because of borrow shenanigans
                    self.state_profile.insert(state, dt);
                }
                if !run { break; }
            }
        } else {
            while self.step(shared) {
            }
        }
    }

    #[inline(never)]
    fn bad_char_error(&mut self, c: &SingleChar) {
        self.bad_char_error_simple(c.decode_as_char())
    }

    #[inline(never)]
    fn bad_char_error_simple(&mut self, c: char) {
        let _ignored_when_for_c = c;
        let msg = format_if!(
            self.opts.exact_errors,
            "Bad character",
            "Saw {} in state {}", c, self.state);
        self.emit_error(msg);
    }

    #[inline(never)]
    fn bad_eof_error(&mut self) {
        let msg = format_if!(
            self.opts.exact_errors,
            "Unexpected EOF",
            "Saw EOF in state {}", self.state);
        self.emit_error(msg);
    }

    #[inline]
    fn emit_unicode_replacement(&mut self, _: SingleChar) {
        self.emit_char(SingleChar::unicode_replacement())
    }

    #[inline(never)]
    fn emit_null(&mut self, _: SingleChar) {
        self.process_token(NullCharacterToken);
    }

    #[inline]
    fn emit_char(&mut self, c: SingleChar) {
        self.process_token(CharacterTokens(c.into_span()))
    }

    #[inline]
    fn emit_buf(&mut self, buf: Buf) {
        self.process_token(CharacterTokens(BufSpan::from_buf(buf)));
    }

    // The string must not contain '\0'!
    #[inline]
    fn emit_span(&mut self, b: Span) {
        self.process_token(CharacterTokens(b));
    }

    fn push_doctype_name_unicode_replacement(&mut self, _: SingleChar) {
        option_push(&mut self.current_doctype_name, SingleChar::unicode_replacement())
    }

    fn push_doctype_id_unicode_replacement(&mut self, k: states::DoctypeIdKind, _: SingleChar) {
        option_push(self.doctype_id(k), SingleChar::unicode_replacement())
    }

    fn push_tag_unicode_replacement(&mut self) {
        self.current_tag_name.push('\ufffd')
    }

    fn emit_current_tag(&mut self) {
        self.finish_attribute();
        let name = Atom::from_slice(self.current_tag_name.as_slice());
        self.current_tag_name.truncate(0);

        match self.current_tag_kind {
            StartTag => {
                self.last_start_tag_name = Some(name.clone());
            }
            EndTag => {
                if !self.current_tag_attrs.is_empty() {
                    self.emit_error(Slice("Attributes on an end tag"));
                }
                if self.current_tag_self_closing {
                    self.emit_error(Slice("Self-closing end tag"));
                }
            }
        }

        let token = TagToken(Tag {
            kind: self.current_tag_kind,
            name: name,
            self_closing: self.current_tag_self_closing,
            attrs: replace(&mut self.current_tag_attrs, vec!()),
        });
        self.process_token(token);

        if self.current_tag_kind == StartTag {
            match self.sink.query_state_change() {
                None => (),
                Some(s) => self.state = s,
            }
        }
    }

    fn push_value_unicode_replacement(&mut self, _: SingleChar) {
        self.current_attr_value.push_sc(SingleChar::unicode_replacement())
    }

    fn emit_temp_buf(&mut self) {
        // FIXME: Make sure that clearing on emit is spec-compatible.
        let span = replace(&mut self.temp_buf, BufSpan::new());
        self.emit_span(span);
    }

    fn clear_temp_buf(&mut self) {
        self.temp_buf = BufSpan::new();
    }

    fn emit_another_temp_buf(&mut self) {
        let span = replace(&mut self.another_temp_buf, BufSpan::new());
        self.emit_span(span);
    }

    fn clear_another_temp_buf(&mut self) {
        self.replace_another_temp_buf_span(BufSpan::new());
    }

    fn replace_another_temp_buf(&mut self, c: SingleChar) {
        self.replace_another_temp_buf_span(c.into_span());
    }

    fn replace_another_temp_buf_span(&mut self, s: Span) {
        self.another_temp_buf = s;
    }

    fn append_another_temp_buf_to_comment(&mut self) {
        self.current_comment.append(replace(&mut self.another_temp_buf, BufSpan::new()));
    }

    fn push_comment_unicode_replacement(&mut self, _: SingleChar) {
        self.current_comment.push_sc(SingleChar::unicode_replacement());
    }

    fn clear_comment_end_dashes(&mut self) {
        self.first_comment_end_dash  = None;
        self.second_comment_end_dash = None;
    }

    fn push_comment_end_dash(&mut self, c: SingleChar) {
        match replace(&mut self.second_comment_end_dash, Some(c.into_buf())) {
            None => {},
            Some(old_second) => {
                match replace(&mut self.first_comment_end_dash, Some(old_second)) {
                    None => {},
                    Some(old_first) => {
                        self.current_comment.push(old_first);
                    }
                }
            }
        }
    }

    fn flush_comment_end_dashes_to_comment(&mut self) {
        match self.first_comment_end_dash.take() {
            None => {},
            Some(buf) => self.current_comment.push(buf),
        }
        match self.second_comment_end_dash.take() {
            None => {},
            Some(buf) => self.current_comment.push(buf),
        }
    }

    fn emit_current_comment(&mut self) {
        let span = replace(&mut self.current_comment, BufSpan::new());
        self.process_token(CommentToken(span));
    }

    fn discard_tag(&mut self) {
        self.current_tag_name.truncate(0);
        self.current_tag_self_closing = false;
        self.current_tag_attrs = vec!();
    }

    fn create_tag(&mut self, kind: TagKind, c: char) {
        self.current_tag_name.truncate(0);
        self.current_tag_name.push(c);
        self.current_tag_self_closing = false;
        self.current_tag_attrs = vec!();
        self.current_tag_kind = kind;
    }

    #[inline]
    fn create_start_tag(&mut self, c: char) {
        self.create_tag(StartTag, c)
    }

    #[inline]
    fn create_end_tag(&mut self, c: char) {
        self.create_tag(EndTag, c)
    }

    fn have_appropriate_end_tag(&self) -> bool {
        match self.last_start_tag_name.as_ref() {
            Some(last) =>
                (self.current_tag_kind == EndTag)
                && self.current_tag_name.as_slice() == last.as_slice(),
            None => false,
        }
    }

    fn create_attribute(&mut self, c: char) {
        self.finish_attribute();

        self.current_attr_name.push(c);
    }

    fn create_attribute_unicode_replacement(&mut self) {
        self.create_attribute('\ufffd');
    }

    fn finish_attribute(&mut self) {
        if self.current_attr_name.is_empty() {
            return;
        }

        // Check for a duplicate attribute.
        // FIXME: the spec says we should error as soon as the name is finished.
        // FIXME: linear time search, do we care?
        let dup = {
            let name = self.current_attr_name.as_slice();
            self.current_tag_attrs.iter().any(|a| a.name.local.as_slice() == name)
        };

        if dup {
            self.emit_error(Slice("Duplicate attribute"));
            self.current_attr_value = BufSpan::new();
        } else {
            self.current_tag_attrs.push(Attribute {
                // The tree builder will adjust the namespace if necessary.
                // This only happens in foreign elements.
                name: QualName::new(ns!(""), Atom::from_slice(self.current_attr_name.as_slice())),
                value: replace(&mut self.current_attr_value, BufSpan::new()),
            });
        }

        self.current_attr_name.truncate(0);
    }

    fn push_name_unicode_replacement(&mut self) {
        self.current_attr_name.push('\ufffd');
    }

    fn create_doctype(&mut self) {
        self.current_doctype      = Doctype::new();
        self.current_doctype_name = None;
    }

    fn emit_current_doctype(&mut self) {
        let mut doctype = replace(&mut self.current_doctype, Doctype::new());

        doctype.name =
            replace(&mut self.current_doctype_name, None)
            .map(|name| name.with_lower_str_copy(Atom::from_slice));

        self.process_token(DoctypeToken(doctype));
    }

    fn doctype_id<'a>(&'a mut self, kind: DoctypeIdKind) -> &'a mut Option<Span> {
        match kind {
            Public => &mut self.current_doctype.public_id,
            System => &mut self.current_doctype.system_id,
        }
    }

    fn clear_doctype_id(&mut self, kind: DoctypeIdKind) {
        *self.doctype_id(kind) = Some(BufSpan::new());
    }

    fn consume_char_ref(&mut self, amp: SingleChar, addnl_allowed: Option<u8>) {
        // NB: The char ref tokenizer assumes we have an additional allowed
        // character iff we're tokenizing in an attribute value.
        self.char_ref_tokenizer = Some(box CharRefTokenizer::new(amp, addnl_allowed));
    }

    fn emit_eof(&mut self) {
        self.process_token(EOFToken);
    }

    fn peek(&mut self, dst: &mut FastOption<SingleChar>) -> OptValue {
        match self.current_char {
            Some(ref c) => dst.fill((*c).clone()),
            None => self.input_buffers.peek(dst),
        }
    }

    fn discard_char(&mut self) {
        let mut c = FastOption::new();
        match self.get_char(&mut c) {
            Uninit => panic!("Should be discarding a valid char."),
            Full   => {},
        }
    }

    fn unconsume(&mut self, span: Span) {
        for buf in span.into_iter().rev() {
            self.input_buffers.push_front(buf);
        }
    }

    fn emit_error(&mut self, error: MaybeOwned<'static>) {
        self.process_token(ParseError(error));
    }
}
//§ END

// Shorthand for common state machine behaviors.
macro_rules! shorthand (
    ( $me:expr : emit_null $c:expr                  ) => ( $me.emit_null($c.take());                               );
    ( $me:expr : emit $c:expr                       ) => ( $me.emit_char($c.take());                               );
    ( $me:expr : emit_ur $c:expr                    ) => ( $me.emit_unicode_replacement($c.take());                );
    ( $me:expr : emit_buf $b:expr                   ) => ( $me.emit_buf($b.take());                                );
    ( $me:expr : emit_span $s:expr                  ) => ( $me.emit_span($s.take());                               );
    ( $me:expr : emit_span_raw $s:expr              ) => ( $me.emit_span($s);                                      );
    ( $me:expr : create_start_tag $c:expr           ) => ( $me.create_start_tag(lower_ascii($c as char));          );
    ( $me:expr : create_end_tag $c:expr             ) => ( $me.create_end_tag(lower_ascii($c as char));            );
    ( $me:expr : push_tag $c:expr                   ) => ( go!($me: push_tag_char ($c as char));                   );
    ( $me:expr : push_tag_char $c:expr              ) => ( $me.current_tag_name.push(lower_ascii($c));             );
    ( $me:expr : push_tag_ur                        ) => ( $me.push_tag_unicode_replacement();                     );
    ( $me:expr : discard_tag                        ) => ( $me.discard_tag();                                      );
    ( $me:expr : push_temp $c:expr                  ) => ( $me.temp_buf.push_sc($c.take());                        );
    ( $me:expr : push_temp_clone $c:expr            ) => ( $me.temp_buf.push_sc((*$c.as_ref()).clone());           );
    ( $me:expr : emit_temp                          ) => ( $me.emit_temp_buf();                                    );
    ( $me:expr : clear_temp                         ) => ( $me.clear_temp_buf();                                   );
    ( $me:expr : push_temp2 $c: expr                ) => ( $me.another_temp_buf.push_sc($c.take());                );
    ( $me:expr : push_temp2_span $s: expr           ) => ( $me.another_temp_buf.append($s.take());                 );
    ( $me:expr : emit_temp2                         ) => ( $me.emit_another_temp_buf();                            );
    ( $me:expr : clear_temp2                        ) => ( $me.clear_another_temp_buf();                           );
    ( $me:expr : replace_temp2 $c: expr             ) => ( $me.replace_another_temp_buf($c.take());                );
    ( $me:expr : replace_temp2_span $s: expr        ) => ( $me.replace_another_temp_buf_span($s.take());           );
    ( $me:expr : append_temp2_to_comment            ) => ( $me.append_another_temp_buf_to_comment();               );
    ( $me:expr : clear_comment_end_dashes           ) => ( $me.clear_comment_end_dashes();                         );
    ( $me:expr : push_comment_end_dash $c: expr     ) => ( $me.push_comment_end_dash($c.take());                   );
    ( $me:expr : flush_comment_end_dashes           ) => ( $me.flush_comment_end_dashes_to_comment();              );
    ( $me:expr : create_attr $c:expr                ) => ( $me.create_attribute(lower_ascii($c));                  );
    ( $me:expr : create_attr_ur                     ) => ( $me.create_attribute_unicode_replacement();             );
    ( $me:expr : push_name $c:expr                  ) => ( $me.current_attr_name.push(lower_ascii($c));            );
    ( $me:expr : push_name_ur                       ) => ( $me.push_name_unicode_replacement();                    );
    ( $me:expr : append_value $b:expr               ) => ( $me.current_attr_value.push($b.take());                 );
    ( $me:expr : push_value $c:expr                 ) => ( $me.current_attr_value.push_sc($c.take());              );
    ( $me:expr : push_value_ur $c:expr              ) => ( $me.push_value_unicode_replacement($c.take());          );
    ( $me:expr : append_value_span $s:expr          ) => ( $me.current_attr_value.append($s.take());               );
    ( $me:expr : append_value_span_raw $s:expr      ) => ( $me.current_attr_value.append($s);                      );
    ( $me:expr : push_comment $c:expr               ) => ( $me.current_comment.push_sc($c.take());                 );
    ( $me:expr : push_comment_ur $c:expr            ) => ( $me.push_comment_unicode_replacement($c.take());        );
    ( $me:expr : append_comment $c:expr             ) => ( $me.current_comment.append($c.take());                  );
    ( $me:expr : emit_comment                       ) => ( $me.emit_current_comment();                             );
    ( $me:expr : clear_comment                      ) => ( $me.current_comment = BufSpan::new();                   );
    ( $me:expr : create_doctype                     ) => ( $me.create_doctype();                                   );
    ( $me:expr : push_doctype_name $c:expr          ) => ( option_push(&mut $me.current_doctype_name, $c.take());  );
    ( $me:expr : push_doctype_name_ur $c:expr       ) => ( $me.push_doctype_name_unicode_replacement($c.take());   );
    ( $me:expr : push_doctype_id $k:expr $c:expr    ) => ( option_push($me.doctype_id($k), $c.take());             );
    ( $me:expr : push_doctype_id_ur $k:expr $c:expr ) => ( $me.push_doctype_id_unicode_replacement($k, $c.take()); );
    ( $me:expr : clear_doctype_id $k:expr           ) => ( $me.clear_doctype_id($k);                               );
    ( $me:expr : force_quirks                       ) => ( $me.current_doctype.force_quirks = true;                );
    ( $me:expr : emit_doctype                       ) => ( $me.emit_current_doctype();                             );
    ( $me:expr : error $c:expr                      ) => ( $me.bad_char_error($c.as_ref());                        );
    ( $me:expr : error_simple $c:expr               ) => ( $me.bad_char_error_simple($c);                          );
    ( $me:expr : error_raw $c:expr                  ) => ( $me.bad_char_error(&$c);                                );
    ( $me:expr : error_eof                          ) => ( $me.bad_eof_error();                                    );
)

// Tracing of tokenizer actions.  This adds significant bloat and compile time,
// so it's behind a cfg flag.
#[cfg(trace_tokenizer)]
macro_rules! sh_trace ( ( $me:expr : $($cmds:tt)* ) => ({
    h5e_debug!("  {}", stringify!($($cmds)*));
    shorthand!($me:expr : $($cmds)*);
}))

#[cfg(not(trace_tokenizer))]
macro_rules! sh_trace ( ( $me:expr : $($cmds:tt)* ) => ( shorthand!($me: $($cmds)*) ) )

// A little DSL for sequencing shorthand actions.
macro_rules! go (
    // A pattern like $($cmd:tt)* ; $($rest:tt)* causes parse ambiguity.
    // We have to tell the parser how much lookahead we need.

    ( $me:expr : $a:tt                   ; $($rest:tt)* ) => ({ sh_trace!($me: $a);          go!($me: $($rest)*); });
    ( $me:expr : $a:tt $b:tt             ; $($rest:tt)* ) => ({ sh_trace!($me: $a $b);       go!($me: $($rest)*); });
    ( $me:expr : $a:tt $b:tt $c:tt       ; $($rest:tt)* ) => ({ sh_trace!($me: $a $b $c);    go!($me: $($rest)*); });
    ( $me:expr : $a:tt $b:tt $c:tt $d:tt ; $($rest:tt)* ) => ({ sh_trace!($me: $a $b $c $d); go!($me: $($rest)*); });

    // These can only come at the end.

    ( $me:expr : to $s:ident                   ) => ({ $me.state = states::$s; return true;           });
    ( $me:expr : to $s:ident $k1:expr          ) => ({ $me.state = states::$s($k1); return true;      });
    ( $me:expr : to $s:ident $k1:expr $k2:expr ) => ({ $me.state = states::$s($k1($k2)); return true; });

    ( $me:expr : reconsume $c:expr $s:ident                   ) => ({ $me.current_char = Some($c.take()); go!($me: to $s);         });
    ( $me:expr : reconsume $c:expr $s:ident $k1:expr          ) => ({ $me.current_char = Some($c.take()); go!($me: to $s $k1);     });
    ( $me:expr : reconsume $c:expr $s:ident $k1:expr $k2:expr ) => ({ $me.current_char = Some($c.take()); go!($me: to $s $k1 $k2); });

    ( $me:expr : consume_char_ref $amp:expr             ) => ({ $me.consume_char_ref($amp.take(), None);         return true; });
    ( $me:expr : consume_char_ref $amp:expr $addnl:expr ) => ({ $me.consume_char_ref($amp.take(), Some($addnl)); return true; });

    // We have a default next state after emitting a tag, but the sink can override.
    ( $me:expr : emit_tag $s:ident ) => ({
        $me.state = states::$s;
        $me.emit_current_tag();
        return true;
    });

    ( $me:expr : eof ) => ({ $me.emit_eof(); return false; });

    // If nothing else matched, it's a single command
    ( $me:expr : $($cmd:tt)+ ) => ( sh_trace!($me: $($cmd)+); );

    // or nothing.
    ($me:expr : ) => (());
)

macro_rules! go_match ( ( $me:expr : $x:expr, $($pats:pat)|+ => $($cmds:tt)* ) => (
    match $x {
        $($pats)|+ => go!($me: $($cmds)*),
        _ => (),
    }
))

// This is a macro because it can cause early return
// from the function where it is used.
macro_rules! get_char ( ($me:expr, $s:expr) => (
    {
        match $me.get_char(&mut $s.c) {
            Uninit => { return false },
            Full   => {},
        };
        ($s.c).as_ref().as_u8()
    }
))

macro_rules! get_char_simple ( ($me:expr) => (
    match $me.get_char_simple() { None => return false, Some(c) => c }
))

macro_rules! pop_except_from ( ($me:expr, $s:expr, $set:expr) => (
    {
        let ret = $me.pop_except_from($set, &mut $s.c, &mut $s.r);
        match ret {
            (Uninit, Uninit) => return false,
            ret => ret,
        }
    }
))

macro_rules! eat ( ($me:expr, $pat:expr) => (
    unwrap_or_return!($me.eat($pat), false)
))

// Clean up the state machine type signatures a little bit.
type TI<Sink> = TokenizerInner<Sink>;

impl<Sink: TokenSink> TokenizerInner<Sink> {
    // Run the state machine for a while.
    // Return true if we should be immediately re-invoked
    // (this just simplifies control flow vs. break / continue).
    #[inline(always)]
    fn step(&mut self, s: &mut Shared) -> bool {
        if self.char_ref_tokenizer.is_some() {
            return self.step_char_ref_tokenizer();
        }

        h5e_debug!("processing in state {}", self.state);
        match self.state {
            //§ data-state
            states::Data => {
                #[inline(never)]
                fn data_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match pop_except_from!(this, s, small_char_set!('\r' '\0' '&' '<')) {
                            (_, Full) => go!(this: emit_buf s.r),
                                  _   => match s.c.as_ref().as_u8() {
                                b'<'  => go!(this: replace_temp2 s.c; to TagOpen),
                                b'&'  => go!(this: consume_char_ref s.c),
                                b'\0' => go!(this: error s.c; emit_null s.c),
                                  _   => go!(this: emit s.c),
                            },
                        }
                    }
                }
                data_state(self, s)
            },

            //§ rcdata-state
            states::RawData(Rcdata) => {
                #[inline(never)]
                fn rcdata_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match pop_except_from!(this, s, small_char_set!('\r' '\0' '&' '<')) {
                            (Full, Uninit) => match s.c.as_ref().as_u8() {
                                b'\0' => go!(this: error s.c; emit_ur s.c),
                                b'&'  => go!(this: consume_char_ref s.c),
                                b'<'  => go!(this: replace_temp2 s.c; to RawLessThanSign Rcdata),
                                  _   => go!(this: emit s.c),
                            },
                            (Uninit, Full) => go!(this: emit_buf s.r),
                            _ => unreachable!(),
                        }
                    }
                }
                rcdata_state(self, s)
            },

            //§ rawtext-state
            states::RawData(Rawtext) => {
                #[inline(never)]
                fn rawtext_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match pop_except_from!(this, s, small_char_set!('\r' '\0' '<')) {
                            (Full, Uninit) => match s.c.as_ref().as_u8() {
                                b'\0' => go!(this: error s.c; emit_ur s.c),
                                b'<'  => go!(this: replace_temp2 s.c; to RawLessThanSign Rawtext),
                                  _   => go!(this: emit s.c),
                            },
                            (Uninit, Full) => go!(this: emit_buf s.r),
                            _ => unreachable!(),
                        }
                    }
                }
                rawtext_state(self, s)
            },

            //§ script-data-state
            states::RawData(ScriptData) => {
                #[inline(never)]
                fn script_data_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match pop_except_from!(this, s, small_char_set!('\r' '\0' '<')) {
                            (Full, Uninit) => match s.c.as_ref().as_u8() {
                                b'\0' => go!(this: error s.c; emit_ur s.c),
                                b'<'  => go!(this: replace_temp2 s.c; to RawLessThanSign ScriptData),
                                  _   => go!(this: emit s.c),
                            },
                            (Uninit, Full) => go!(this: emit_buf s.r),
                            _ => unreachable!(),
                        }
                    }
                }
                script_data_state(self, s)
            },

            //§ script-data-escaped-state
            states::RawData(ScriptDataEscaped(Escaped)) => {
                #[inline(never)]
                fn script_data_escaped_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match pop_except_from!(this, s, small_char_set!('\r' '\0' '-' '<')) {
                            (Full, Uninit) => match s.c.as_ref().as_u8() {
                                b'\0' => go!(this: error s.c; emit_ur s.c),
                                b'-'  => go!(this: emit s.c; to ScriptDataEscapedDash Escaped),
                                b'<'  => go!(this: replace_temp2 s.c; to RawLessThanSign ScriptDataEscaped Escaped),
                                  _   => go!(this: emit s.c),
                            },
                            (Uninit, Full) => go!(this: emit_buf s.r),
                            _ => unreachable!(),
                        }
                    }
                }
                script_data_escaped_state(self, s)
            },

            //§ script-data-double-escaped-state
            states::RawData(ScriptDataEscaped(DoubleEscaped)) => {
                #[inline(never)]
                fn script_data_double_escaped_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match pop_except_from!(this, s, small_char_set!('\r' '\0' '-' '<')) {
                            (Full, Uninit) => match s.c.as_ref().as_u8() {
                                b'\0' => go!(this: error s.c; emit_ur s.c),
                                b'-'  => go!(this: emit s.c; to ScriptDataEscapedDash DoubleEscaped),
                                b'<'  => go!(this: emit s.c; to RawLessThanSign ScriptDataEscaped DoubleEscaped),
                                  _   => go!(this: emit s.c),
                            },
                            (Uninit, Full) => go!(this: emit_buf s.r),
                            _ => unreachable!(),
                        }
                    }
                }
                script_data_double_escaped_state(self, s)
            },

            //§ plaintext-state
            states::Plaintext => {
                #[inline(never)]
                fn plaintext_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match pop_except_from!(this, s, small_char_set!('\r' '\0')) {
                            (Full, Uninit) => match s.c.as_ref().as_u8() {
                                b'\0' => go!(this: error s.c; emit_ur s.c),
                                  _   => go!(this: emit s.c),
                            },
                            (Uninit, Full) => go!(this: emit_buf s.r),
                            _ => unreachable!(),
                        }
                    }
                }
                plaintext_state(self, s)
            },

            //§ tag-open-state
            states::TagOpen => {
                #[inline(never)]
                fn tag_open_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'!' => go!(this: push_temp2 s.c; to MarkupDeclarationOpen),
                            b'/' => go!(this: push_temp2 s.c; to EndTagOpen),
                            b'?' => go!(this: error s.c; clear_comment; push_comment s.c; to BogusComment),
                            chr if is_alphabetic(chr) => go!(this: create_start_tag chr; to TagName),
                            _ => go!(this: error s.c; emit_temp2; reconsume s.c Data)
                        }
                    }
                }
                tag_open_state(self, s)
            },

            //§ end-tag-open-state
            states::EndTagOpen => {
                #[inline(never)]
                fn end_tag_open_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'>'  => go!(this: error s.c; to Data),
                            b'\0' => go!(this: error s.c; clear_comment; push_comment_ur s.c; to BogusComment),
                            chr if is_alphabetic(chr) => go!(this: create_end_tag chr; to TagName),
                            _ => go!(this: error s.c; clear_comment; push_comment s.c; to BogusComment),
                        }
                    }
                }
                end_tag_open_state(self, s)
            },

            //§ tag-name-state
            states::TagName => {
                #[inline(never)]
                fn tag_name_state<S: TokenSink>(this: &mut TI<S>, _s: &mut Shared) -> bool {
                    loop {
                        let c = get_char_simple!(this);
                        match c {
                            '\t' | '\n' | '\x0C' | ' '
                                  => go!(this: to BeforeAttributeName),
                            '/'   => go!(this: to SelfClosingStartTag),
                            '>'   => go!(this: emit_tag Data),
                            '\0'  => go!(this: error_simple c; push_tag_ur),
                              c   => go!(this: push_tag_char c)
                        }
                    }
                }
                tag_name_state(self, s)
            },

            //§ script-data-escaped-less-than-sign-state
            states::RawLessThanSign(ScriptDataEscaped(Escaped)) => {
                #[inline(never)]
                fn script_data_escaped_less_than_sign_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'/' => go!(this: clear_temp; push_temp2 s.c; to RawEndTagOpen ScriptDataEscaped Escaped),
                            chr if is_alphabetic(chr) => go!(this: clear_temp; push_temp_clone s.c; emit_temp2; emit s.c; to ScriptDataEscapeStart DoubleEscaped),
                            _    => go!(this: emit_temp2; reconsume s.c RawData ScriptDataEscaped Escaped),
                        }
                    }
                }
                script_data_escaped_less_than_sign_state(self, s)
            },

            //§ script-data-double-escaped-less-than-sign-state
            states::RawLessThanSign(ScriptDataEscaped(DoubleEscaped)) => {
                #[inline(never)]
                fn script_data_double_escaped_less_than_sign_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'/' => go!(this: clear_temp; emit s.c; to ScriptDataDoubleEscapeEnd),
                              _  => go!(this: reconsume s.c RawData ScriptDataEscaped DoubleEscaped),
                        }
                    }
                }
                script_data_double_escaped_less_than_sign_state(self, s)
            },

            //§ rcdata-less-than-sign-state rawtext-less-than-sign-state script-data-less-than-sign-state
            // otherwise
            states::RawLessThanSign(kind) => {
                #[inline(never)]
                fn other_less_than_sign_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared, kind: RawKind) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'/' => go!(this: clear_temp; push_temp2 s.c; to RawEndTagOpen kind),
                            b'!' if kind == ScriptData => go!(this: emit_temp2; emit s.c; to ScriptDataEscapeStart Escaped),
                              _  => go!(this: emit_temp2; reconsume s.c RawData kind),
                        }
                    }
                }
                other_less_than_sign_state(self, s, kind)
            },

            //§ rcdata-end-tag-open-state rawtext-end-tag-open-state script-data-end-tag-open-state script-data-escaped-end-tag-open-state
            states::RawEndTagOpen(kind) => {
                #[inline(never)]
                fn other_end_tag_open_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared, kind: RawKind) -> bool {
                    loop {
                        match get_char!(this, s) {
                            chr if is_alphabetic(chr) => go!(this: push_temp_clone s.c; create_end_tag chr; to RawEndTagName kind),
                            _                         => go!(this: emit_temp2; reconsume s.c RawData kind)
                        }
                    }
                }
                other_end_tag_open_state(self, s, kind)
            },

            //§ rcdata-end-tag-name-state rawtext-end-tag-name-state script-data-end-tag-name-state script-data-escaped-end-tag-name-state
            states::RawEndTagName(kind) => {
                #[inline(never)]
                fn other_end_tag_name_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared, kind: RawKind) -> bool {
                    loop {
                        let c_u8 = get_char!(this, s);

                        if this.have_appropriate_end_tag() {
                            match c_u8 {
                                b'\t' | b'\n' | b'\x0C' | b' '
                                     => go!(this: to BeforeAttributeName),
                                b'/' => go!(this: to SelfClosingStartTag),
                                b'>' => go!(this: emit_tag Data),
                                _    => {},
                            }
                        }

                        if is_alphabetic(c_u8) {
                            go!(this: push_temp_clone s.c; push_tag c_u8)
                        } else {
                            go!(this: discard_tag; emit_temp2; emit_temp; reconsume s.c RawData kind)
                        }
                    }
                }
                other_end_tag_name_state(self, s, kind)
            },

            //§ script-data-double-escape-start-state
            states::ScriptDataEscapeStart(DoubleEscaped) => {
                #[inline(never)]
                fn script_data_double_escape_start_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        let chr = get_char!(this, s);
                        match chr {
                            b'\t' | b'\n' | b'\x0C' | b' ' | b'/' | b'>' => {
                                let esc = if this.temp_buf.byte_equal_slice_lower(b"script") { DoubleEscaped } else { Escaped };
                                go!(this: emit s.c; to RawData ScriptDataEscaped esc);
                            }
                            chr if is_alphabetic(chr) => go!(this: push_temp_clone s.c; emit s.c),
                            _ => go!(this: reconsume s.c RawData ScriptDataEscaped Escaped),
                        }
                    }
                }
                script_data_double_escape_start_state(self, s)
            },

            //§ script-data-escape-start-state
            states::ScriptDataEscapeStart(Escaped) => {
                #[inline(never)]
                fn script_data_escape_start_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'-' => go!(this: emit s.c; to ScriptDataEscapeStartDash),
                              _  => go!(this: reconsume s.c RawData ScriptData),
                        }
                    }
                }
                script_data_escape_start_state(self, s)
            },

            //§ script-data-escape-start-dash-state
            states::ScriptDataEscapeStartDash => {
                #[inline(never)]
                fn script_data_escape_start_dash_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'-' => go!(this: emit s.c; to ScriptDataEscapedDashDash Escaped),
                              _  => go!(this: reconsume s.c RawData ScriptData),
                        }
                    }
                }
                script_data_escape_start_dash_state(self, s)
            },

            //§ script-data-escaped-dash-state script-data-double-escaped-dash-state
            states::ScriptDataEscapedDash(kind) => {
                #[inline(never)]
                fn script_data_escaped_dash_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared, kind: ScriptEscapeKind) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'-'  => go!(this: emit s.c; to ScriptDataEscapedDashDash kind),
                            b'<' if kind == DoubleEscaped => go!(this: emit s.c; to RawLessThanSign ScriptDataEscaped kind),
                            b'<'                          => go!(this: replace_temp2 s.c; to RawLessThanSign ScriptDataEscaped kind),
                            b'\0' => go!(this: error s.c; emit_ur s.c; to RawData ScriptDataEscaped kind),
                              _   => go!(this: emit s.c; to RawData ScriptDataEscaped kind),
                        }
                    }
                }
                script_data_escaped_dash_state(self, s, kind)
            },

            //§ script-data-escaped-dash-dash-state script-data-double-escaped-dash-dash-state
            states::ScriptDataEscapedDashDash(kind) => {
                #[inline(never)]
                fn script_data_escaped_dash_dash_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared, kind: ScriptEscapeKind) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'-'  => go!(this: emit s.c),
                            b'<' if kind == DoubleEscaped => go!(this: emit s.c; to RawLessThanSign ScriptDataEscaped kind),
                            b'<'                          => go!(this: replace_temp2 s.c; to RawLessThanSign ScriptDataEscaped kind),
                            b'>'  => go!(this: emit s.c; to RawData ScriptData),
                            b'\0' => go!(this: error s.c; emit_ur s.c; to RawData ScriptDataEscaped kind),
                              _   => go!(this: emit s.c; to RawData ScriptDataEscaped kind),
                        }
                    }
                }
                script_data_escaped_dash_dash_state(self, s, kind)
            },

            //§ script-data-double-escape-end-state
            states::ScriptDataDoubleEscapeEnd => {
                #[inline(never)]
                fn script_data_double_escape_end_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        let chr = get_char!(this, s);
                        match chr {
                            b'\t' | b'\n' | b'\x0C' | b' ' | b'/' | b'>' => {
                                let esc = if this.temp_buf.byte_equal_slice_lower(b"script") { Escaped } else { DoubleEscaped };
                                go!(this: emit s.c; to RawData ScriptDataEscaped esc);
                            }
                            chr if is_alphabetic(chr) => go!(this: push_temp_clone s.c; emit s.c),
                            _ => go!(this: reconsume s.c RawData ScriptDataEscaped DoubleEscaped),
                        }
                    }
                }
                script_data_double_escape_end_state(self, s)
            },

            //§ before-attribute-name-state
            states::BeforeAttributeName => {
                #[inline(never)]
                fn before_attribute_name_state<S: TokenSink>(this: &mut TI<S>, _s: &mut Shared) -> bool {
                    loop {
                        let c = get_char_simple!(this);
                        match c {
                            '\t' | '\n' | '\x0C' | ' ' => {},
                            '/'  => go!(this: to SelfClosingStartTag),
                            '>'  => go!(this: emit_tag Data),
                            '\0' => go!(this: error_simple c; create_attr_ur; to AttributeName),
                            chr => {
                                go_match!(this: chr,
                                    '"' | '\'' | '<' | '=' => error_simple chr);
                                go!(this: create_attr c; to AttributeName)
                            }
                        }
                    }
                }
                before_attribute_name_state(self, s)
            },

            //§ attribute-name-state
            states::AttributeName => {
                #[inline(never)]
                fn attribute_name_state<S: TokenSink>(this: &mut TI<S>, _s: &mut Shared) -> bool {
                    loop {
                        let c = get_char_simple!(this);
                        match c {
                            '\t' | '\n' | '\x0C' | ' '
                                 => go!(this: to AfterAttributeName),
                            '/'  => go!(this: to SelfClosingStartTag),
                            '='  => go!(this: to BeforeAttributeValue),
                            '>'  => go!(this: emit_tag Data),
                            '\0' => go!(this: error_simple c; push_name_ur),
                            chr => {
                                go_match!(this: chr,
                                    '"' | '\'' | '<' => error_simple chr);
                                go!(this: push_name chr);
                            }
                        }
                    }
                }
                attribute_name_state(self, s)
            },

            //§ after-attribute-name-state
            states::AfterAttributeName => {
                #[inline(never)]
                fn after_attribute_name_state<S: TokenSink>(this: &mut TI<S>, _s: &mut Shared) -> bool {
                    loop {
                        let c = get_char_simple!(this);
                        match c {
                            '\t' | '\n' | '\x0C' | ' ' => {},
                            '/'  => go!(this: to SelfClosingStartTag),
                            '='  => go!(this: to BeforeAttributeValue),
                            '>'  => go!(this: emit_tag Data),
                            '\0' => go!(this: error_simple c; create_attr_ur; to AttributeName),
                            chr => {
                                go_match!(this: chr,
                                    '"' | '\'' | '<' => error_simple chr);
                                go!(this: create_attr chr; to AttributeName);
                            }
                        }
                    }
                }
                after_attribute_name_state(self, s)
            },

            //§ before-attribute-value-state
            states::BeforeAttributeValue => {
                #[inline(never)]
                fn before_attribute_value_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'\t' | b'\n' | b'\x0C' | b' ' => {},
                            b'"'  => go!(this: to AttributeValue DoubleQuoted),
                            b'&'  => go!(this: reconsume s.c AttributeValue Unquoted),
                            b'\'' => go!(this: to AttributeValue SingleQuoted),
                            b'\0' => go!(this: error s.c; push_value_ur s.c; to AttributeValue Unquoted),
                            b'>'  => go!(this: error s.c; emit_tag Data),
                            chr => {
                                go_match!(this: chr,
                                    b'<' | b'=' | b'`' => error s.c);
                                go!(this: push_value s.c; to AttributeValue Unquoted);
                            }
                        }
                    }
                }
                before_attribute_value_state(self, s)
            },

            //§ attribute-value-(double-quoted)-state
            states::AttributeValue(DoubleQuoted) => {
                #[inline(never)]
                fn attribute_value_double_quoted_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match pop_except_from!(this, s, small_char_set!('\r' '"' '&' '\0')) {
                            (Full, Uninit) => {
                                match s.c.as_ref().as_u8() {
                                    b'"'  => go!(this: to AfterAttributeValueQuoted),
                                    b'&'  => go!(this: consume_char_ref s.c b'"'),
                                    b'\0' => go!(this: error s.c; push_value_ur s.c),
                                    _     => go!(this: push_value s.c),
                                }
                            },
                            (Uninit, Full) => go!(this: append_value s.r),
                            _ => unreachable!(),
                        }
                    }
                }
                attribute_value_double_quoted_state(self, s)
            },

            //§ attribute-value-(single-quoted)-state
            states::AttributeValue(SingleQuoted) => {
                #[inline(never)]
                fn attribute_value_single_quoted_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match pop_except_from!(this, s, small_char_set!('\r' '\'' '&' '\0')) {
                            (Full, Uninit) => {
                                match s.c.as_ref().as_u8() {
                                    b'\'' => go!(this: to AfterAttributeValueQuoted),
                                    b'&'  => go!(this: consume_char_ref s.c b'\''),
                                    b'\0' => go!(this: error s.c; push_value_ur s.c),
                                    _     => go!(this: push_value s.c),
                                }
                            },
                            (Uninit, Full) => go!(this: append_value s.r),
                            _ => unreachable!(),
                        }
                    }
                }
                attribute_value_single_quoted_state(self, s)
            },

            //§ attribute-value-(unquoted)-state
            states::AttributeValue(Unquoted) => {
                #[inline(never)]
                fn attribute_value_unquoted_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match pop_except_from!(this, s, small_char_set!('\r' '\t' '\n' '\x0C' ' ' '&' '>' '\0')) {
                            (Full, Uninit) => {
                                match s.c.as_ref().as_u8() {
                                    b'\t' | b'\n' | b'\x0C' | b' ' => go!(this: to BeforeAttributeName),
                                    b'&'  => go!(this: consume_char_ref s.c b'>'),
                                    b'>'  => go!(this: emit_tag Data),
                                    b'\0' => go!(this: error s.c; push_value_ur s.c),
                                    chr => {
                                        go_match!(this: chr,
                                            b'"' | b'\'' | b'<' | b'=' | b'`' => error s.c);
                                        go!(this: push_value s.c);
                                    }
                                }
                            },
                            (Uninit, Full) => go!(this: append_value s.r),
                            _ => unreachable!(),
                        }
                    }
                }
                attribute_value_unquoted_state(self, s)
            },

            //§ after-attribute-value-(quoted)-state
            states::AfterAttributeValueQuoted => {
                #[inline(never)]
                fn after_attribute_value_quoted_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'\t' | b'\n' | b'\x0C' | b' '
                                  => go!(this: to BeforeAttributeName),
                            b'/'  => go!(this: to SelfClosingStartTag),
                            b'>'  => go!(this: emit_tag Data),
                            _     => go!(this: error s.c; reconsume s.c BeforeAttributeName),
                        }
                    }
                }
                after_attribute_value_quoted_state(self, s)
            },

            //§ self-closing-start-tag-state
            states::SelfClosingStartTag => {
                #[inline(never)]
                fn self_closing_start_tag_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'>' => {
                                this.current_tag_self_closing = true;
                                go!(this: emit_tag Data);
                            }
                            _ => go!(this: error s.c; reconsume s.c BeforeAttributeName),
                        }
                    }
                }
                self_closing_start_tag_state(self, s)
            },

            //§ comment-start-state
            states::CommentStart => {
                #[inline(never)]
                fn comment_start_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'-'  => go!(this: clear_comment_end_dashes; push_comment_end_dash s.c; to CommentStartDash),
                            b'\0' => go!(this: error s.c; push_comment_ur s.c; to Comment),
                            b'>'  => go!(this: error s.c; emit_comment; to Data),
                            _     => go!(this: push_comment s.c; to Comment),
                        }
                    }
                }
                comment_start_state(self, s)
            },

            //§ comment-start-dash-state
            states::CommentStartDash => {
                #[inline(never)]
                fn comment_start_dash_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'-'  => go!(this: push_comment_end_dash s.c; to CommentEnd),
                            b'\0' => go!(this: error s.c; flush_comment_end_dashes; push_comment_ur s.c; to Comment),
                            b'>'  => go!(this: error s.c; emit_comment; to Data),
                            _     => go!(this: flush_comment_end_dashes; push_comment s.c; to Comment),
                        }
                    }
                }
                comment_start_dash_state(self, s)
            },

            //§ comment-state
            states::Comment => {
                #[inline(never)]
                fn comment_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'-'  => go!(this: clear_comment_end_dashes; push_comment_end_dash s.c; to CommentEndDash),
                            b'\0' => go!(this: error s.c; push_comment_ur s.c),
                            _     => go!(this: append_temp2_to_comment; push_comment s.c),
                        }
                    }
                }
                comment_state(self, s)
            },

            //§ comment-end-dash-state
            states::CommentEndDash => {
                #[inline(never)]
                fn comment_end_dash_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'-'  => go!(this: push_comment_end_dash s.c; to CommentEnd),
                            b'\0' => go!(this: error s.c; flush_comment_end_dashes; push_comment_ur s.c; to Comment),
                            _     => go!(this: flush_comment_end_dashes; push_comment s.c; to Comment),
                        }
                    }
                }
                comment_end_dash_state(self, s)
            },

            //§ comment-end-state
            states::CommentEnd => {
                #[inline(never)]
                fn comment_end_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'>'  => go!(this: emit_comment; to Data),
                            b'\0' => go!(this: error s.c; flush_comment_end_dashes; push_comment_ur s.c; to Comment),
                            b'!'  => go!(this: error s.c; clear_temp2; push_temp2 s.c; to CommentEndBang),
                            b'-'  => go!(this: error s.c; push_comment_end_dash s.c),
                            _     => go!(this: error s.c; flush_comment_end_dashes; push_comment s.c; to Comment),
                        }
                    }
                }
                comment_end_state(self, s)
            },

            //§ comment-end-bang-state
            states::CommentEndBang => {
                #[inline(never)]
                fn comment_end_bang_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'-'  => go!(this: flush_comment_end_dashes; append_temp2_to_comment; to CommentEndDash),
                            b'>'  => go!(this: emit_comment; to Data),
                            b'\0' => go!(this: error s.c; flush_comment_end_dashes; append_temp2_to_comment; push_comment_ur s.c; to Comment),
                            _     => go!(this: flush_comment_end_dashes; append_temp2_to_comment; push_comment s.c; to Comment),
                        }
                    }
                }
                comment_end_bang_state(self, s)
            },

            //§ doctype-state
            states::Doctype => {
                #[inline(never)]
                fn doctype_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'\t' | b'\n' | b'\x0C' | b' '
                                => go!(this: to BeforeDoctypeName),
                            _   => go!(this: error s.c; reconsume s.c BeforeDoctypeName),
                        }
                    }
                }
                doctype_state(self, s)
            },

            //§ before-doctype-name-state
            states::BeforeDoctypeName => {
                #[inline(never)]
                fn before_doctype_name_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'\t' | b'\n' | b'\x0C' | b' ' => {},
                            b'\0' => go!(this: error s.c; create_doctype; push_doctype_name_ur s.c; to DoctypeName),
                            b'>'  => go!(this: error s.c; create_doctype; force_quirks; emit_doctype; to Data),
                            _     => go!(this: create_doctype; push_doctype_name s.c; to DoctypeName),
                        }
                    }
                }
                before_doctype_name_state(self, s)
            },

            //§ doctype-name-state
            states::DoctypeName => {
                #[inline(never)]
                fn doctype_name_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'\t' | b'\n' | b'\x0C' | b' '
                                  => go!(this: to AfterDoctypeName),
                            b'>'  => go!(this: emit_doctype; to Data),
                            b'\0' => go!(this: error s.c; push_doctype_name_ur s.c),
                            _     => go!(this: push_doctype_name s.c),
                        }
                    }
                }
                doctype_name_state(self, s)
            },

            //§ after-doctype-name-state
            states::AfterDoctypeName => {
                #[inline(never)]
                fn after_doctype_name_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        if !eat!(this, b"public").is_empty() {
                            go!(this: to AfterDoctypeKeyword Public);
                        } else if !eat!(this, b"system").is_empty() {
                            go!(this: to AfterDoctypeKeyword System);
                        } else {
                            match get_char!(this, s) {
                                b'\t' | b'\n' | b'\x0C' | b' ' => {},
                                b'>' => go!(this: emit_doctype; to Data),
                                _    => go!(this: error s.c; force_quirks; to BogusDoctype),
                            }
                        }
                    }
                }
                after_doctype_name_state(self, s)
            },

            //§ after-doctype-public-keyword-state after-doctype-system-keyword-state
            states::AfterDoctypeKeyword(kind) => {
                #[inline(never)]
                fn after_doctype_keyword_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared, kind: DoctypeIdKind) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'\t' | b'\n' | b'\x0C' | b' '
                                 => go!(this: to BeforeDoctypeIdentifier kind),
                            b'"'  => go!(this: error s.c; clear_doctype_id kind; to DoctypeIdentifierDoubleQuoted kind),
                            b'\'' => go!(this: error s.c; clear_doctype_id kind; to DoctypeIdentifierSingleQuoted kind),
                            b'>'  => go!(this: error s.c; force_quirks; emit_doctype; to Data),
                            _     => go!(this: error s.c; force_quirks; to BogusDoctype),
                        }
                    }
                }
                after_doctype_keyword_state(self, s, kind)
            },

            //§ before-doctype-public-identifier-state before-doctype-system-identifier-state
            states::BeforeDoctypeIdentifier(kind) => {
                #[inline(never)]
                fn before_doctype_identifier_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared, kind: DoctypeIdKind) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'\t' | b'\n' | b'\x0C' | b' ' => {},
                            b'"'  => go!(this: clear_doctype_id kind; to DoctypeIdentifierDoubleQuoted kind),
                            b'\'' => go!(this: clear_doctype_id kind; to DoctypeIdentifierSingleQuoted kind),
                            b'>'  => go!(this: error s.c; force_quirks; emit_doctype; to Data),
                            _     => go!(this: error s.c; force_quirks; to BogusDoctype),
                        }
                    }
                }
                before_doctype_identifier_state(self, s, kind)
            },

            //§ doctype-public-identifier-(double-quoted)-state doctype-system-identifier-(double-quoted)-state
            states::DoctypeIdentifierDoubleQuoted(kind) => {
                #[inline(never)]
                fn doctype_identifier_double_quoted_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared, kind: DoctypeIdKind) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'"'  => go!(this: to AfterDoctypeIdentifier kind),
                            b'\0' => go!(this: error s.c; push_doctype_id_ur kind s.c),
                            b'>'  => go!(this: error s.c; force_quirks; emit_doctype; to Data),
                            _     => go!(this: push_doctype_id kind s.c),
                        }
                    }
                }
                doctype_identifier_double_quoted_state(self, s, kind)
            },

            //§ doctype-public-identifier-(single-quoted)-state doctype-system-identifier-(single-quoted)-state
            states::DoctypeIdentifierSingleQuoted(kind) => {
                #[inline(never)]
                fn doctype_identifier_single_quoted_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared, kind: DoctypeIdKind) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'\'' => go!(this: to AfterDoctypeIdentifier kind),
                            b'\0' => go!(this: error s.c; push_doctype_id_ur kind s.c),
                            b'>'  => go!(this: error s.c; force_quirks; emit_doctype; to Data),
                            _     => go!(this: push_doctype_id kind s.c),
                        }
                    }
                }
                doctype_identifier_single_quoted_state(self, s, kind)
            },

            //§ after-doctype-public-identifier-state
            states::AfterDoctypeIdentifier(Public) => {
                #[inline(never)]
                fn after_doctype_public_identifier_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'\t' | b'\n' | b'\x0C' | b' '
                                 => go!(this: to BetweenDoctypePublicAndSystemIdentifiers),
                            b'>'  => go!(this: emit_doctype; to Data),
                            b'"'  => go!(this: error s.c; clear_doctype_id System; to DoctypeIdentifierDoubleQuoted System),
                            b'\'' => go!(this: error s.c; clear_doctype_id System; to DoctypeIdentifierSingleQuoted System),
                            _     => go!(this: error s.c; force_quirks; to BogusDoctype),
                        }
                    }
                }
                after_doctype_public_identifier_state(self, s)
            },

            //§ after-doctype-system-identifier-state
            states::AfterDoctypeIdentifier(System) => {
                #[inline(never)]
                fn after_doctype_system_identifier_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'\t' | b'\n' | b'\x0C' | b' ' => {},
                            b'>' => go!(this: emit_doctype; to Data),
                            _    => go!(this: error s.c; to BogusDoctype),
                        }
                    }
                }
                after_doctype_system_identifier_state(self, s)
            },

            //§ between-doctype-public-and-system-identifiers-state
            states::BetweenDoctypePublicAndSystemIdentifiers => {
                #[inline(never)]
                fn between_doctype_public_and_system_identifiers_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'\t' | b'\n' | b'\x0C' | b' ' => {},
                            b'>'  => go!(this: emit_doctype; to Data),
                            b'"'  => go!(this: clear_doctype_id System; to DoctypeIdentifierDoubleQuoted System),
                            b'\'' => go!(this: clear_doctype_id System; to DoctypeIdentifierSingleQuoted System),
                            _     => go!(this: error s.c; force_quirks; to BogusDoctype),
                        }
                    }
                }
                between_doctype_public_and_system_identifiers_state(self, s)
            },

            //§ bogus-doctype-state
            states::BogusDoctype => {
                #[inline(never)]
                fn bogus_doctype_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'>'  => go!(this: emit_doctype; to Data),
                            _     => {},
                        }
                    }
                }
                bogus_doctype_state(self, s)
            },

            //§ bogus-comment-state
            states::BogusComment => {
                #[inline(never)]
                fn bogus_comment_state<S: TokenSink>(this: &mut TI<S>, s: &mut Shared) -> bool {
                    loop {
                        match get_char!(this, s) {
                            b'>'  => go!(this: emit_comment; to Data),
                            b'\0' => go!(this: push_comment_ur s.c),
                            _     => go!(this: push_comment s.c),
                        }
                    }
                }
                bogus_comment_state(self, s)
            },

            //§ markup-declaration-open-state
            states::MarkupDeclarationOpen => {
                #[inline(never)]
                fn markup_declaration_open_state<S: TokenSink>(this: &mut TI<S>, _s: &mut Shared) -> bool {
                    loop {
                        let span = eat!(this, b"--");
                        if !span.is_empty() {
                            go!(this: clear_temp2; clear_comment; to CommentStart);
                        }

                        let span = eat!(this, b"doctype");
                        if !span.is_empty() {
                            go!(this: to Doctype);
                        }

                        // FIXME: CDATA, requires "adjusted current node" from tree builder
                        // FIXME: 'error' gives wrong message
                        let nil = SingleChar::null();
                        let c = (*this.current_char.as_ref().unwrap_or(&nil)).clone();
                        go!(this: error_raw c; to BogusComment);
                    }
                }
                markup_declaration_open_state(self, s)
            },

            //§ cdata-section-state
            states::CdataSection => {
                #[inline(never)]
                fn cdata_section_state<S: TokenSink>(this: &mut TI<S>, _s: &mut Shared) -> bool {
                    panic!("FIXME: state {} not implemented", this.state)
                }
                cdata_section_state(self, s)
            }
            //§ END
        }
    }

    fn step_char_ref_tokenizer(&mut self) -> bool {
        // FIXME HACK: Take and replace the tokenizer so we don't
        // double-mut-borrow self.  This is why it's boxed.
        let mut tok = self.char_ref_tokenizer.take().unwrap();
        let outcome = tok.step(self);

        let progress = match outcome {
            char_ref::Done => {
                self.process_char_ref(tok.get_result());
                return true;
            }

            char_ref::Stuck => false,
            char_ref::Progress => true,
        };

        self.char_ref_tokenizer = Some(tok);
        progress
    }

    #[inline(never)]
    fn process_char_ref(&mut self, chars: Span) {
        match self.state {
            states::Data | states::RawData(states::Rcdata) => go!(self: emit_span_raw chars),
            states::AttributeValue(_) => go!(self: append_value_span_raw chars),
            _ => panic!("state {} should not be reachable in process_char_ref", self.state),
        }
    }

    /// Indicate that we have reached the end of the input.
    fn end(&mut self, shared: &mut Shared) {
        // Handle EOF in the char ref sub-tokenizer, if there is one.
        // Do this first because it might un-consume stuff.
        match self.char_ref_tokenizer.take() {
            None => (),
            Some(mut tok) => {
                tok.end_of_file(self);
                self.process_char_ref(tok.get_result());
            }
        }

        // Process all remaining buffered input.
        // If we're waiting for lookahead, we're not gonna get it.
        self.at_eof = true;
        self.run(shared);

        while self.eof_step() {
            // loop
        }

        if self.opts.profile {
            self.dump_profile();
        }
    }

    #[cfg(for_c)]
    fn dump_profile(&self) {
        unreachable!();
    }

    #[cfg(not(for_c))]
    fn dump_profile(&self) {
        use core::iter::AdditiveIterator;

        let mut results: Vec<(states::State, u64)>
            = self.state_profile.iter().map(|(s, t)| (*s, *t)).collect();
        results.as_mut_slice().sort_by(|&(_, x), &(_, y)| y.cmp(&x));

        let total = results.iter().map(|&(_, t)| t).sum();
        println!("\nTokenizer profile, in nanoseconds");
        println!("\n{:12}         total in token sink", self.time_in_sink);
        println!("\n{:12}         total in tokenizer", total);

        for (k, v) in results.into_iter() {
            let pct = 100.0 * (v as f64) / (total as f64);
            println!("{:12}  {:4.1}%  {}", v, pct, k);
        }
    }

    fn eof_step(&mut self) -> bool {
        h5e_debug!("processing EOF in state {}", self.state);
        match self.state {
            states::Data | states::RawData(Rcdata) | states::RawData(Rawtext)
            | states::RawData(ScriptData) | states::Plaintext
                => go!(self: eof),

            states::TagName | states::RawData(ScriptDataEscaped(_))
            | states::BeforeAttributeName | states::AttributeName
            | states::AfterAttributeName | states::BeforeAttributeValue
            | states::AttributeValue(_) | states::AfterAttributeValueQuoted
            | states::SelfClosingStartTag | states::ScriptDataEscapedDash(_)
            | states::ScriptDataEscapedDashDash(_)
                => go!(self: error_eof; to Data),

            states::TagOpen
                => go!(self: error_eof; emit_temp2; to Data),

            states::EndTagOpen
                => go!(self: error_eof; emit_temp2; to Data),

            states::RawLessThanSign(ScriptDataEscaped(DoubleEscaped))
                => go!(self: to RawData ScriptDataEscaped DoubleEscaped),

            states::RawLessThanSign(kind)
                => go!(self: emit_temp2; to RawData kind),

            states::RawEndTagOpen(kind)
                => {
                    go!(self: emit_temp2; to RawData kind)
            },

            states::RawEndTagName(kind)
                => {
                    go!(self: emit_temp2; emit_temp; to RawData kind)
            },

            states::ScriptDataEscapeStart(kind)
                => go!(self: to RawData ScriptDataEscaped kind),

            states::ScriptDataEscapeStartDash
                => go!(self: to RawData ScriptData),

            states::ScriptDataDoubleEscapeEnd
                => go!(self: to RawData ScriptDataEscaped DoubleEscaped),

            states::CommentStart | states::CommentStartDash
            | states::Comment | states::CommentEndDash
            | states::CommentEnd | states::CommentEndBang
                => go!(self: error_eof; emit_comment; to Data),

            states::Doctype | states::BeforeDoctypeName
                => go!(self: error_eof; create_doctype; force_quirks; emit_doctype; to Data),

            states::DoctypeName | states::AfterDoctypeName | states::AfterDoctypeKeyword(_)
            | states::BeforeDoctypeIdentifier(_) | states::DoctypeIdentifierDoubleQuoted(_)
            | states::DoctypeIdentifierSingleQuoted(_) | states::AfterDoctypeIdentifier(_)
            | states::BetweenDoctypePublicAndSystemIdentifiers
                => go!(self: error_eof; force_quirks; emit_doctype; to Data),

            states::BogusDoctype
                => go!(self: emit_doctype; to Data),

            states::BogusComment
                => go!(self: emit_comment; to Data),

            states::MarkupDeclarationOpen
                => {
                    let c = (*self.current_char.as_ref().unwrap()).clone();
                    go!(self: error_raw c; to BogusComment)
                },

            states::CdataSection
                => panic!("FIXME: state {} not implemented in EOF", self.state),
        }
    }
}
