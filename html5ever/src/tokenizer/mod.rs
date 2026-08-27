// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The HTML5 tokenizer.

pub use self::interface::{CharacterTokens, EOFToken, NullCharacterToken, ParseError};
pub use self::interface::{CommentToken, DoctypeToken, TagToken, Token};
pub use self::interface::{Doctype, EndTag, StartTag, Tag, TagKind};
pub use self::interface::{TokenSink, TokenSinkResult};

use self::states::AttrValueKind::*;
use self::states::DoctypeIdKind::{self, *};
use self::states::RawKind::*;
use self::states::ScriptEscapeKind::*;
use self::states::State::{self, *};

use self::char_ref::{CharRef, CharRefTokenizer};

use crate::util::str::lower_ascii_letter;

use log::{debug, trace};
use markup5ever::{ns, small_char_set, TokenizerResult};
use std::borrow::Cow::{self, Borrowed};
use std::cell::{Cell, RefCell, RefMut};
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::mem;

pub use crate::buffer_queue::{BufferQueue, FromSet, NotFromSet, SetResult};
use crate::macros::{time, unwrap_or_return};
use crate::tendril::StrTendril;
use crate::{Attribute, LocalName, QualName, SmallCharSet};

mod char_ref;
mod interface;
pub mod states;

/// The result of invoking the tokenizer once.
pub enum ProcessResult<Handle> {
    /// The tokenizer should be re-invoked immediately.
    Continue,
    /// The tokenizer has not finished, but it needs to wait for more
    /// input to arrive before it can continue.
    Suspend,
    /// The tokenizer was blocked by a `<script>`.
    ///
    /// This `<script>` needs to be executed before tokenization
    /// can continue, as it might invoke `document.write`.
    Script(Handle),
    /// The tokenizer was blocked because it found a `<meta charset>` tag.
    ///
    /// Such tags may force the user agent to re-parse the document with the new
    /// encoding, but non-conformant implementations can reasonably treat
    /// this as [Self::Continue].
    EncodingIndicator(StrTendril),
}

fn option_push(opt_str: &mut Option<StrTendril>, c: char) {
    match *opt_str {
        Some(ref mut s) => s.push_char(c),
        None => *opt_str = Some(StrTendril::from_char(c)),
    }
}

/// Tokenizer options, with an impl for `Default`.
#[derive(Clone)]
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
    ///
    /// FIXME: Can't use Tendril because we want TokenizerOpts
    /// to be Send.
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

/// The HTML tokenizer.
pub struct Tokenizer<Sink> {
    /// Options controlling the behavior of the tokenizer.
    opts: TokenizerOpts,

    /// Destination for tokens we emit.
    pub sink: Sink,

    /// The abstract machine state as described in the spec.
    state: Cell<states::State>,

    /// Are we at the end of the file, once buffers have been processed
    /// completely? This affects whether we will wait for lookahead or not.
    at_eof: Cell<bool>,

    /// Tokenizer for character references, if we're tokenizing
    /// one at the moment.
    char_ref_tokenizer: RefCell<Option<CharRefTokenizer>>,

    /// Current input character.  Just consumed, may reconsume.
    current_char: Cell<char>,

    /// Should we reconsume the current input character?
    reconsume: Cell<bool>,

    /// Did we just consume \r, translating it to \n?  In that case we need
    /// to ignore the next character if it's \n.
    ignore_lf: Cell<bool>,

    /// Discard a U+FEFF BYTE ORDER MARK if we see one?  Only done at the
    /// beginning of the stream.
    discard_bom: Cell<bool>,

    /// Current tag kind.
    current_tag_kind: Cell<TagKind>,

    /// Current tag name.
    current_tag_name: RefCell<StrTendril>,

    /// Current tag is self-closing?
    current_tag_self_closing: Cell<bool>,

    /// Current tag had duplicate attributes?
    current_tag_had_duplicate_attributes: Cell<bool>,

    /// Current tag attributes.
    current_tag_attrs: RefCell<Vec<Attribute>>,

    /// Current attribute name.
    current_attr_name: RefCell<StrTendril>,

    /// Current attribute value.
    current_attr_value: RefCell<StrTendril>,

    /// Current comment.
    current_comment: RefCell<StrTendril>,

    /// Current doctype token.
    current_doctype: RefCell<Doctype>,

    /// Last start tag name, for use in checking "appropriate end tag".
    last_start_tag_name: RefCell<Option<LocalName>>,

    /// The "temporary buffer" mentioned in the spec.
    temp_buf: RefCell<StrTendril>,

    /// Record of how many ns we spent in each state, if profiling is enabled.
    state_profile: RefCell<BTreeMap<states::State, u64>>,

    /// Record of how many ns we spent in the token sink.
    time_in_sink: Cell<u64>,

    /// Track current line
    current_line: Cell<u64>,
}

impl<Sink: TokenSink> Tokenizer<Sink> {
    /// Create a new tokenizer which feeds tokens to a particular `TokenSink`.
    pub fn new(sink: Sink, mut opts: TokenizerOpts) -> Tokenizer<Sink> {
        let start_tag_name = opts
            .last_start_tag_name
            .take()
            .map(|s| LocalName::from(&*s));
        let state = opts.initial_state.unwrap_or(states::Data);
        let discard_bom = opts.discard_bom;
        Tokenizer {
            opts,
            sink,
            state: Cell::new(state),
            char_ref_tokenizer: RefCell::new(None),
            at_eof: Cell::new(false),
            current_char: Cell::new('\0'),
            reconsume: Cell::new(false),
            ignore_lf: Cell::new(false),
            discard_bom: Cell::new(discard_bom),
            current_tag_kind: Cell::new(StartTag),
            current_tag_name: RefCell::new(StrTendril::new()),
            current_tag_self_closing: Cell::new(false),
            current_tag_had_duplicate_attributes: Cell::new(false),
            current_tag_attrs: RefCell::new(vec![]),
            current_attr_name: RefCell::new(StrTendril::new()),
            current_attr_value: RefCell::new(StrTendril::new()),
            current_comment: RefCell::new(StrTendril::new()),
            current_doctype: RefCell::new(Doctype::default()),
            last_start_tag_name: RefCell::new(start_tag_name),
            temp_buf: RefCell::new(StrTendril::new()),
            state_profile: RefCell::new(BTreeMap::new()),
            time_in_sink: Cell::new(0),
            current_line: Cell::new(1),
        }
    }

    /// Feed an input string into the tokenizer.
    pub fn feed(&self, input: &BufferQueue) -> TokenizerResult<Sink::Handle> {
        if input.is_empty() {
            return TokenizerResult::Done;
        }

        if self.discard_bom.get() {
            if let Some(c) = input.peek() {
                if c == '\u{feff}' {
                    input.next();
                }
            } else {
                return TokenizerResult::Done;
            }
        };

        self.run(input)
    }

    pub fn set_plaintext_state(&self) {
        self.state.set(states::Plaintext);
    }

    fn process_token(&self, token: Token) -> TokenSinkResult<Sink::Handle> {
        if self.opts.profile {
            let (ret, dt) = time!(self.sink.process_token(token, self.current_line.get()));
            self.time_in_sink.set(self.time_in_sink.get() + dt);
            ret
        } else {
            self.sink.process_token(token, self.current_line.get())
        }
    }

    fn process_token_and_continue(&self, token: Token) {
        assert!(matches!(
            self.process_token(token),
            TokenSinkResult::Continue
        ));
    }

    //§ preprocessing-the-input-stream
    // Get the next input character, which might be the character
    // 'c' that we already consumed from the buffers.
    fn get_preprocessed_char(&self, mut c: char, input: &BufferQueue) -> Option<char> {
        if self.ignore_lf.get() {
            self.ignore_lf.set(false);
            if c == '\n' {
                c = input.next()?;
            }
        }

        if c == '\r' {
            self.ignore_lf.set(true);
            c = '\n';
        }

        if c == '\n' {
            self.current_line.set(self.current_line.get() + 1);
        }

        if self.opts.exact_errors
            && match c as u32 {
                0x01..=0x08 | 0x0B | 0x0E..=0x1F | 0x7F..=0x9F | 0xFDD0..=0xFDEF => true,
                n if (n & 0xFFFE) == 0xFFFE => true,
                _ => false,
            }
        {
            let msg = format!("Bad character {c}");
            self.emit_error(Cow::Owned(msg));
        }

        trace!("got character {c}");
        self.current_char.set(c);
        Some(c)
    }

    //§ tokenization
    // Get the next input character, if one is available.
    fn get_char(&self, input: &BufferQueue) -> Option<char> {
        if self.reconsume.get() {
            self.reconsume.set(false);
            Some(self.current_char.get())
        } else {
            input
                .next()
                .and_then(|c| self.get_preprocessed_char(c, input))
        }
    }

    fn pop_except_from(&self, input: &BufferQueue, set: SmallCharSet) -> Option<SetResult> {
        // Bail to the slow path for various corner cases.
        // This means that `FromSet` can contain characters not in the set!
        // It shouldn't matter because the fallback `FromSet` case should
        // always do the same thing as the `NotFromSet` case.
        if self.opts.exact_errors || self.reconsume.get() || self.ignore_lf.get() {
            return self.get_char(input).map(FromSet);
        }

        let d = input.pop_except_from(set);
        trace!("got characters {d:?}");
        match d {
            Some(FromSet(c)) => self.get_preprocessed_char(c, input).map(FromSet),

            // NB: We don't set self.current_char for a run of characters not
            // in the set.  It shouldn't matter for the codepaths that use
            // this.
            _ => d,
        }
    }

    // Check if the next characters are an ASCII case-insensitive match.  See
    // BufferQueue::eat.
    //
    // NB: this doesn't set the current input character.
    fn eat(&self, input: &BufferQueue, pat: &str, eq: fn(&u8, &u8) -> bool) -> Option<bool> {
        if self.ignore_lf.get() {
            self.ignore_lf.set(false);
            if self.peek(input) == Some('\n') {
                self.discard_char(input);
            }
        }

        input.push_front(mem::take(&mut self.temp_buf.borrow_mut()));
        match input.eat(pat, eq) {
            None if self.at_eof.get() => Some(false),
            None => {
                while let Some(data) = input.next() {
                    self.temp_buf.borrow_mut().push_char(data);
                }
                None
            },
            Some(matched) => Some(matched),
        }
    }

    /// Run the state machine for as long as we can.
    fn run(&self, input: &BufferQueue) -> TokenizerResult<Sink::Handle> {
        if self.opts.profile {
            loop {
                let state = self.state.get();
                let old_sink = self.time_in_sink.get();
                let (run, mut dt) = time!(self.step(input));
                dt -= (self.time_in_sink.get() - old_sink);
                let new = match self.state_profile.borrow_mut().get_mut(&state) {
                    Some(x) => {
                        *x += dt;
                        false
                    },
                    None => true,
                };
                if new {
                    // do this here because of borrow shenanigans
                    self.state_profile.borrow_mut().insert(state, dt);
                }
                match run {
                    ProcessResult::Continue => (),
                    ProcessResult::Suspend => break,
                    ProcessResult::Script(node) => return TokenizerResult::Script(node),
                    ProcessResult::EncodingIndicator(encoding) => {
                        return TokenizerResult::EncodingIndicator(encoding)
                    },
                }
            }
        } else {
            loop {
                match self.step(input) {
                    ProcessResult::Continue => (),
                    ProcessResult::Suspend => break,
                    ProcessResult::Script(node) => return TokenizerResult::Script(node),
                    ProcessResult::EncodingIndicator(encoding) => {
                        return TokenizerResult::EncodingIndicator(encoding)
                    },
                }
            }
        }
        TokenizerResult::Done
    }

    #[inline]
    fn bad_char_error(&self) {
        #[cfg(feature = "trace_tokenizer")]
        trace!("  error");

        let msg = if self.opts.exact_errors {
            Cow::from("Bad character")
        } else {
            let c = self.current_char.get();
            let state = self.state.get();
            Cow::from(format!("Saw {c} in state {state:?}"))
        };
        self.emit_error(msg);
    }

    #[inline]
    fn bad_eof_error(&self) {
        #[cfg(feature = "trace_tokenizer")]
        trace!("  error_eof");

        let msg = if self.opts.exact_errors {
            Cow::from("Unexpected EOF")
        } else {
            let state = self.state.get();
            Cow::from(format!("Saw EOF in state {state:?}"))
        };
        self.emit_error(msg);
    }

    fn emit_char(&self, c: char) {
        #[cfg(feature = "trace_tokenizer")]
        trace!("  emit");

        self.process_token_and_continue(match c {
            '\0' => NullCharacterToken,
            _ => CharacterTokens(StrTendril::from_char(c)),
        });
    }

    // The string must not contain '\0'!
    fn emit_chars(&self, b: StrTendril) {
        self.process_token_and_continue(CharacterTokens(b));
    }

    fn emit_current_tag(&self) -> ProcessResult<Sink::Handle> {
        self.finish_attribute();

        let name = LocalName::from(&**self.current_tag_name.borrow());
        self.current_tag_name.borrow_mut().clear();

        match self.current_tag_kind.get() {
            StartTag => {
                *self.last_start_tag_name.borrow_mut() = Some(name.clone());
            },
            EndTag => {
                if !self.current_tag_attrs.borrow().is_empty() {
                    self.emit_error(Borrowed("Attributes on an end tag"));
                }
                if self.current_tag_self_closing.get() {
                    self.emit_error(Borrowed("Self-closing end tag"));
                }
            },
        }

        let token = TagToken(Tag {
            kind: self.current_tag_kind.get(),
            name,
            self_closing: self.current_tag_self_closing.get(),
            attrs: std::mem::take(&mut self.current_tag_attrs.borrow_mut()),
            had_duplicate_attributes: self.current_tag_had_duplicate_attributes.get(),
        });

        match self.process_token(token) {
            TokenSinkResult::Continue => ProcessResult::Continue,
            TokenSinkResult::Plaintext => {
                self.state.set(states::Plaintext);
                ProcessResult::Continue
            },
            TokenSinkResult::Script(node) => {
                self.state.set(states::Data);
                ProcessResult::Script(node)
            },
            TokenSinkResult::RawData(kind) => {
                self.state.set(states::RawData(kind));
                ProcessResult::Continue
            },
            TokenSinkResult::EncodingIndicator(encoding) => {
                ProcessResult::EncodingIndicator(encoding)
            },
        }
    }

    fn emit_temp_buf(&self) {
        #[cfg(feature = "trace_tokenizer")]
        trace!("  emit_temp");

        // FIXME: Make sure that clearing on emit is spec-compatible.
        let buf = mem::take(&mut *self.temp_buf.borrow_mut());
        self.emit_chars(buf);
    }

    fn clear_temp_buf(&self) {
        // Do this without a new allocation.
        self.temp_buf.borrow_mut().clear();
    }

    fn emit_current_comment(&self) {
        let comment = mem::take(&mut *self.current_comment.borrow_mut());
        self.process_token_and_continue(CommentToken(comment));
    }

    fn discard_tag(&self) {
        self.current_tag_name.borrow_mut().clear();
        self.current_tag_self_closing.set(false);
        self.current_tag_had_duplicate_attributes.set(false);
        *self.current_tag_attrs.borrow_mut() = vec![];
    }

    fn create_tag(&self, kind: TagKind, c: char) {
        self.discard_tag();
        self.current_tag_name.borrow_mut().push_char(c);
        self.current_tag_kind.set(kind);
    }

    fn have_appropriate_end_tag(&self) -> bool {
        match self.last_start_tag_name.borrow().as_ref() {
            Some(last) => {
                (self.current_tag_kind.get() == EndTag)
                    && (**self.current_tag_name.borrow() == **last)
            },
            None => false,
        }
    }

    fn create_attribute(&self, c: char) {
        self.finish_attribute();

        self.current_attr_name.borrow_mut().push_char(c);
    }

    fn finish_attribute(&self) {
        if self.current_attr_name.borrow().is_empty() {
            return;
        }
        let name = LocalName::from(&**self.current_attr_name.borrow());
        self.current_attr_name.borrow_mut().clear();
        // Check for a duplicate attribute.
        // FIXME: the spec says we should error as soon as the name is finished.
        let dup = {
            self.current_tag_attrs
                .borrow()
                .iter()
                .any(|a| a.name.local == name)
        };

        if dup {
            self.emit_error(Borrowed("Duplicate attribute"));
            self.current_tag_had_duplicate_attributes.set(true);
            self.current_attr_value.borrow_mut().clear();
        } else {
            self.current_tag_attrs.borrow_mut().push(Attribute {
                // The tree builder will adjust the namespace if necessary.
                // This only happens in foreign elements.
                name: QualName::new(None, ns!(), name),
                value: mem::take(&mut self.current_attr_value.borrow_mut()),
            });
        }
    }

    fn emit_current_doctype(&self) {
        let doctype = self.current_doctype.take();
        self.process_token_and_continue(DoctypeToken(doctype));
    }

    fn doctype_id(&self, kind: DoctypeIdKind) -> RefMut<'_, Option<StrTendril>> {
        let current_doctype = self.current_doctype.borrow_mut();
        match kind {
            Public => RefMut::map(current_doctype, |d| &mut d.public_id),
            System => RefMut::map(current_doctype, |d| &mut d.system_id),
        }
    }

    fn clear_doctype_id(&self, kind: DoctypeIdKind) {
        let mut id = self.doctype_id(kind);
        match *id {
            Some(ref mut s) => s.clear(),
            None => *id = Some(StrTendril::new()),
        }
    }

    fn start_consuming_character_reference(&self) {
        debug_assert!(
            self.char_ref_tokenizer.borrow().is_none(),
            "Nested character references are impossible"
        );

        let is_in_attribute = matches!(self.state.get(), states::AttributeValue(_));
        *self.char_ref_tokenizer.borrow_mut() = Some(CharRefTokenizer::new(is_in_attribute));
    }

    fn emit_eof(&self) {
        self.process_token_and_continue(EOFToken);
    }

    fn peek(&self, input: &BufferQueue) -> Option<char> {
        if self.reconsume.get() {
            Some(self.current_char.get())
        } else {
            input.peek()
        }
    }

    fn discard_char(&self, input: &BufferQueue) {
        // peek() deals in un-processed characters (no newline normalization), while get_char()
        // does.
        //
        // since discard_char is supposed to be used in combination with peek(), discard_char must
        // discard a single raw input character, not a normalized newline.
        if self.reconsume.get() {
            self.reconsume.set(false);
        } else {
            input.next();
        }
    }

    fn emit_error(&self, error: Cow<'static, str>) {
        self.process_token_and_continue(ParseError(error));
    }
}
//§ END

// Shorthand for common state machine behaviors.
macro_rules! shorthand (
    ( $me:ident : create_tag $kind:ident $c:expr   ) => ( $me.create_tag($kind, $c)                           );
    ( $me:ident : push_tag $c:expr                 ) => ( $me.current_tag_name.borrow_mut().push_char($c)     );
    ( $me:ident : discard_tag                      ) => ( $me.discard_tag()                                   );
    ( $me:ident : discard_char $input:expr         ) => ( $me.discard_char($input)                            );
    ( $me:ident : push_temp $c:expr                ) => ( $me.temp_buf.borrow_mut().push_char($c)             );
    ( $me:ident : clear_temp                       ) => ( $me.clear_temp_buf()                                );
    ( $me:ident : create_attr $c:expr              ) => ( $me.create_attribute($c)                            );
    ( $me:ident : push_name $c:expr                ) => ( $me.current_attr_name.borrow_mut().push_char($c)    );
    ( $me:ident : push_value $c:expr               ) => ( $me.current_attr_value.borrow_mut().push_char($c)   );
    ( $me:ident : append_value $c:expr             ) => ( $me.current_attr_value.borrow_mut().push_tendril($c));
    ( $me:ident : push_comment $c:expr             ) => ( $me.current_comment.borrow_mut().push_char($c)      );
    ( $me:ident : append_comment $c:expr           ) => ( $me.current_comment.borrow_mut().push_slice($c)     );
    ( $me:ident : emit_comment                     ) => ( $me.emit_current_comment()                          );
    ( $me:ident : clear_comment                    ) => ( $me.current_comment.borrow_mut().clear()            );
    ( $me:ident : create_doctype                   ) => ( *$me.current_doctype.borrow_mut() = Doctype::default() );
    ( $me:ident : push_doctype_name $c:expr        ) => ( option_push(&mut $me.current_doctype.borrow_mut().name, $c) );
    ( $me:ident : push_doctype_id $k:ident $c:expr ) => ( option_push(&mut $me.doctype_id($k), $c)            );
    ( $me:ident : clear_doctype_id $k:ident        ) => ( $me.clear_doctype_id($k)                            );
    ( $me:ident : force_quirks                     ) => ( $me.current_doctype.borrow_mut().force_quirks = true);
    ( $me:ident : emit_doctype                     ) => ( $me.emit_current_doctype()                          );
);

// Tracing of tokenizer actions.  This adds significant bloat and compile time,
// so it's behind a cfg flag.
#[cfg(feature = "trace_tokenizer")]
macro_rules! sh_trace ( ( $me:ident : $($cmds:tt)* ) => ({
    trace!("  {:?}", stringify!($($cmds)*));
    shorthand!($me : $($cmds)*);
}));

#[cfg(not(feature = "trace_tokenizer"))]
macro_rules! sh_trace ( ( $me:ident : $($cmds:tt)* ) => ( shorthand!($me: $($cmds)*) ) );

// A little DSL for sequencing shorthand actions.
macro_rules! go (
    // A pattern like $($cmd:tt)* ; $($rest:tt)* causes parse ambiguity.
    // We have to tell the parser how much lookahead we need.

    ( $me:ident : $a:tt                   ; $($rest:tt)* ) => ({ sh_trace!($me: $a);          go!($me: $($rest)*); });
    ( $me:ident : $a:tt $b:tt             ; $($rest:tt)* ) => ({ sh_trace!($me: $a $b);       go!($me: $($rest)*); });
    ( $me:ident : $a:tt $b:tt $c:tt       ; $($rest:tt)* ) => ({ sh_trace!($me: $a $b $c);    go!($me: $($rest)*); });
    ( $me:ident : $a:tt $b:tt $c:tt $d:tt ; $($rest:tt)* ) => ({ sh_trace!($me: $a $b $c $d); go!($me: $($rest)*); });

    // These can only come at the end.

    ( $me:ident : to $s:expr                  ) => ({ $me.state.set($s); return ProcessResult::Continue;                });
    ( $me:ident : reconsume $s:expr           ) => ({ $me.reconsume.set(true); go!($me: to $s);                                 });
    ( $me:ident : consume_char_ref             ) => ({ $me.start_consuming_character_reference(); return ProcessResult::Continue;});

    // We have a default next state after emitting a tag, but the sink can override.
    ( $me:ident : emit_tag $s:ident ) => ({
        $me.state.set(states::$s);
        return $me.emit_current_tag();
    });

    ( $me:ident : eof ) => ({ $me.emit_eof(); return ProcessResult::Suspend; });

    // If nothing else matched, it's a single command
    ( $me:ident : $($cmd:tt)+ ) => ( sh_trace!($me: $($cmd)+) );

    // or nothing.
    ( $me:ident : ) => (());
);

// This is a macro because it can cause early return
// from the function where it is used.
macro_rules! get_char ( ($me:expr, $input:expr) => (
    unwrap_or_return!($me.get_char($input), ProcessResult::Suspend)
));

macro_rules! peek ( ($me:expr, $input:expr) => (
    unwrap_or_return!($me.peek($input), ProcessResult::Suspend)
));

macro_rules! eat ( ($me:expr, $input:expr, $pat:expr) => (
    unwrap_or_return!($me.eat($input, $pat, u8::eq_ignore_ascii_case), ProcessResult::Suspend)
));

macro_rules! eat_exact ( ($me:expr, $input:expr, $pat:expr) => (
    unwrap_or_return!($me.eat($input, $pat, u8::eq), ProcessResult::Suspend)
));

impl<Sink: TokenSink> Tokenizer<Sink> {
    // Run the state machine for a while.
    // Return true if we should be immediately re-invoked
    // (this just simplifies control flow vs. break / continue).
    #[allow(clippy::never_loop)]
    fn step(&self, input: &BufferQueue) -> ProcessResult<Sink::Handle> {
        if self.char_ref_tokenizer.borrow().is_some() {
            return self.step_char_ref_tokenizer(input);
        }

        trace!("processing in state {:?}", self.state);
        match self.state.get() {
            // https://html.spec.whatwg.org/#data-state
            states::Data => loop {
                // Step 1. Consume the next input character:
                let set = small_char_set!('\r' '\0' '&' '<' '\n');

                #[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64"))]
                let set_result = if !(self.opts.exact_errors
                    || self.reconsume.get()
                    || self.ignore_lf.get())
                    && Self::is_supported_simd_feature_detected()
                {
                    let front_buffer = input.peek_front_chunk_mut();
                    let Some(mut front_buffer) = front_buffer else {
                        return ProcessResult::Suspend;
                    };

                    // Special case: The fast path is not worth taking if the first character is already in the set,
                    // which is fairly common
                    let first_char = front_buffer
                        .chars()
                        .next()
                        .expect("Input buffers are never empty");

                    if matches!(first_char, '\r' | '\0' | '&' | '<' | '\n') {
                        drop(front_buffer);
                        self.pop_except_from(input, set)
                    } else {
                        // SAFETY:
                        // This CPU is guaranteed to support SIMD due to the is_supported_simd_feature_detected check above
                        let result = unsafe { self.data_state_simd_fast_path(&mut front_buffer) };

                        if front_buffer.is_empty() {
                            drop(front_buffer);
                            input.pop_front();
                        }

                        result
                    }
                } else {
                    self.pop_except_from(input, set)
                };

                #[cfg(not(any(
                    target_arch = "x86",
                    target_arch = "x86_64",
                    target_arch = "aarch64"
                )))]
                let set_result = self.pop_except_from(input, set);

                let Some(set_result) = set_result else {
                    return ProcessResult::Suspend;
                };
                match set_result {
                    // ↪ U+0026 AMPERSAND (&)
                    FromSet('&') => {
                        // Set the return state to the data state. Switch to the character reference state.
                        go!(self: consume_char_ref)
                    },
                    // ↪ U+003C LESS-THAN SIGN (<)
                    FromSet('<') => {
                        // Switch to the tag open state.
                        go!(self: to State::TagOpen)
                    },
                    // ↪ U+0000 NULL
                    FromSet('\0') => {
                        // This is an unexpected-null-character parse error.
                        // Emit the current input character as a character token.
                        self.bad_char_error();
                        self.emit_char('\0');
                    },
                    // ↪ Anything else
                    //     Emit the current input character as a character token.
                    FromSet(character) => self.emit_char(character),
                    NotFromSet(characters) => self.emit_chars(characters),
                }
            },

            // https://html.spec.whatwg.org/#rcdata-state
            states::RawData(Rcdata) => loop {
                // Consume the next input character:
                let Some(set_result) =
                    self.pop_except_from(input, small_char_set!('\r' '\0' '&' '<' '\n'))
                else {
                    return ProcessResult::Suspend;
                };

                match set_result {
                    // ↪ U+0026 AMPERSAND (&)
                    FromSet('&') => {
                        go!(self: consume_char_ref)
                    },
                    // ↪ U+003C LESS-THAN SIGN (<)
                    FromSet('<') => {
                        // Switch to the RCDATA less-than sign state.
                        go!(self: to State::RawLessThanSign(Rcdata))
                    },
                    // ↪ U+0000 NULL
                    FromSet('\0') => {
                        self.bad_char_error();
                        self.emit_char('\u{fffd}');
                    },
                    // ↪ Anything else
                    //     Emit the current input character as a character token.
                    FromSet(character) => self.emit_char(character),
                    NotFromSet(characters) => self.emit_chars(characters),
                }
            },

            // https://html.spec.whatwg.org/#rawtext-state
            states::RawData(Rawtext) => loop {
                // Consume the next input character:
                let Some(set_result) =
                    self.pop_except_from(input, small_char_set!('\r' '\0' '<' '\n'))
                else {
                    return ProcessResult::Suspend;
                };

                match set_result {
                    // ↪ U+003C LESS-THAN SIGN (<)
                    FromSet('<') => {
                        // Switch to the RAWTEXT less-than sign state.
                        go!(self: to State::RawLessThanSign(Rawtext));
                    },
                    // ↪ U+0000 NULL
                    FromSet('\0') => {
                        // This is an unexpected-null-character parse error.
                        // Emit a U+FFFD REPLACEMENT CHARACTER character token.
                        self.bad_char_error();
                        self.emit_char('\u{fffd}');
                    },
                    // ↪ Anything else
                    //     Emit the current input character as a character token.
                    FromSet(character) => self.emit_char(character),
                    NotFromSet(characters) => self.emit_chars(characters),
                }
            },

            // https://html.spec.whatwg.org/#script-data-state
            states::RawData(ScriptData) => loop {
                // Consume the next input character:
                let Some(set_result) =
                    self.pop_except_from(input, small_char_set!('\r' '\0' '<' '\n'))
                else {
                    return ProcessResult::Suspend;
                };

                match set_result {
                    // ↪ U+003C LESS-THAN SIGN (<)
                    FromSet('<') => {
                        // Switch to the script data less-than sign state.
                        go!(self: to State::RawLessThanSign(ScriptData));
                    },
                    // ↪ U+0000 NULL
                    FromSet('\0') => {
                        // This is an unexpected-null-character parse error.
                        // Emit a U+FFFD REPLACEMENT CHARACTER character token.
                        self.bad_char_error();
                        self.emit_char('\u{fffd}');
                    },
                    // ↪ Anything else
                    //     Emit the current input character as a character token.
                    FromSet(character) => self.emit_char(character),
                    NotFromSet(characters) => self.emit_chars(characters),
                }
            },

            // https://html.spec.whatwg.org/#plaintext-state
            states::Plaintext => loop {
                // Consume the next input character:
                let Some(set_result) = self.pop_except_from(input, small_char_set!('\r' '\0' '\n'))
                else {
                    return ProcessResult::Suspend;
                };

                match set_result {
                    // ↪ U+0000 NULL
                    FromSet('\0') => {
                        // This is an unexpected-null-character parse error.
                        // Emit a U+FFFD REPLACEMENT CHARACTER character token.
                        self.bad_char_error();
                        self.emit_char('\u{fffd}');
                    },
                    // ↪ Anything else
                    //     Emit the current input character as a character token.
                    FromSet(character) => self.emit_char(character),
                    NotFromSet(characters) => self.emit_chars(characters),
                }
            },

            // https://html.spec.whatwg.org/#tag-open-state
            states::TagOpen => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0021 EXCLAMATION MARK (!)
                    '!' => {
                        // Switch to the markup declaration open state.
                        go!(self: to State::MarkupDeclarationOpen)
                    },
                    // ↪ U+002F SOLIDUS (/)
                    '/' => {
                        // Switch to the end tag open state.
                        go!(self: to State::EndTagOpen)
                    },
                    // ↪ ASCII alpha
                    character if character.is_ascii_alphabetic() => {
                        // Create a new start tag token, set its tag name to the empty string.
                        // Reconsume in the tag name state.
                        // NOTE: We don't reconsume the character but instead immediately append it in lowercase
                        // to the new tag (as that is what the "tag name" state would do).
                        let character = character.to_ascii_lowercase();
                        go!(self: create_tag StartTag character; to State::TagName)
                    },
                    // ↪ U+003F QUESTION MARK (?)
                    '?' => {
                        // Set the temporary buffer to the empty string.
                        // Switch to the processing instruction open state.
                        self.bad_char_error();
                        go!(self: clear_comment; reconsume BogusComment)
                    },
                    // ↪ Anything else
                    _ => {
                        // This is an invalid-first-character-of-tag-name parse error.
                        // Emit a U+003C LESS-THAN SIGN character token.
                        // Reconsume in the data state.
                        self.bad_char_error();
                        self.emit_char('<');
                        go!(self: reconsume Data)
                    },
                }
            },

            // https://html.spec.whatwg.org/#end-tag-open-state
            states::EndTagOpen => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ ASCII alpha
                    character if character.is_ascii_alphabetic() => {
                        // Create a new end tag token, set its tag name to the empty string.
                        // Reconsume in the tag name state.
                        // NOTE: We don't reconsume the character but instead immediately append in lowercase
                        // to the new tag (as that is what the "tag name" state would do).
                        let character = character.to_ascii_lowercase();
                        go!(self: create_tag EndTag character; to State::TagName)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // This is a missing-end-tag-name parse error.
                        // Switch to the data state.
                        self.bad_char_error();
                        go!(self: to State::Data)
                    },
                    // ↪ Anything else
                    _ => {
                        // This is an invalid-first-character-of-tag-name parse error.
                        // Create a comment token whose data is the empty string.
                        // Reconsume in the bogus comment state.
                        self.bad_char_error();
                        go!(self: clear_comment; reconsume BogusComment)
                    },
                }
            },

            // https://html.spec.whatwg.org/#tag-name-state
            states::TagName => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Switch to the before attribute name state.
                        go!(self: to State::BeforeAttributeName)
                    },
                    // ↪ U+002F SOLIDUS (/)
                    '/' => {
                        // Switch to the self-closing start tag state.
                        go!(self: to State::SelfClosingStartTag)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the data state.
                        // Emit the current tag token.
                        go!(self: emit_tag Data)
                    },
                    // ↪ ASCII upper alpha
                    character if character.is_ascii_uppercase() => {
                        // Append the lowercase version of the current input character (add 0x0020 to the
                        // character's code point) to the current tag token's tag name.
                        go!(self: push_tag (character.to_ascii_lowercase()))
                    },
                    // ↪ U+0000 NULL
                    '\0' => {
                        self.bad_char_error();
                        go!(self: push_tag '\u{fffd}')
                    },
                    // ↪ Anything else
                    character => {
                        // Append the current input character to the current tag token's tag name.
                        go!(self: push_tag (character))
                    },
                }
            },

            // https://html.spec.whatwg.org/#script-data-escaped-less-than-sign-state
            states::RawLessThanSign(ScriptDataEscaped(Escaped)) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002F SOLIDUS (/)
                    '/' => {
                        go!(self: clear_temp; to State::RawEndTagOpen(ScriptDataEscaped(Escaped)))
                    },
                    character => match lower_ascii_letter(character) {
                        // ↪ ASCII alpha
                        Some(character_lowercase) => {
                            // Set the temporary buffer to the empty string.
                            // Emit a U+003C LESS-THAN SIGN character token.
                            // Reconsume in the script data double escape start state.
                            // NOTE: We don't reconsume, and instead emit the lowercased character immediately,
                            // as that is what the "script data double escape start" state would do.
                            go!(self: clear_temp; push_temp character_lowercase);
                            self.emit_char('<');
                            self.emit_char(character);
                            go!(self: to State::ScriptDataEscapeStart(DoubleEscaped));
                        },
                        // ↪ Anything else
                        None => {
                            self.emit_char('<');
                            go!(self: reconsume RawData(ScriptDataEscaped(Escaped)));
                        },
                    },
                }
            },

            // https://html.spec.whatwg.org/#script-data-double-escaped-less-than-sign-state
            states::RawLessThanSign(ScriptDataEscaped(DoubleEscaped)) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002F SOLIDUS (/)
                    '/' => {
                        // Set the temporary buffer to the empty string.
                        // Switch to the script data double escape end state.
                        // Emit a U+002F SOLIDUS character token.
                        go!(self: clear_temp);
                        self.emit_char('/');
                        go!(self: to State::ScriptDataDoubleEscapeEnd);
                    },
                    // ↪ Anything else
                    _ => {
                        // Reconsume in the script data double escaped state.
                        go!(self: reconsume RawData(ScriptDataEscaped(DoubleEscaped)))
                    },
                }
            },

            // https://html.spec.whatwg.org/#rcdata-less-than-sign-state
            // https://html.spec.whatwg.org/#script-data-less-than-sign-state
            // https://html.spec.whatwg.org/#rawtext-less-than-sign-state
            states::RawLessThanSign(kind) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002F SOLIDUS (/)
                    '/' => {
                        // Set the temporary buffer to the empty string.
                        // Switch to the RCDATA end tag open state.
                        go!(self: clear_temp; to State::RawEndTagOpen(kind))
                    },
                    // ↪ U+0021 EXCLAMATION MARK (!)
                    '!' if kind == ScriptData => {
                        // Switch to the script data escape start state.
                        // Emit a U+003C LESS-THAN SIGN character token and a U+0021 EXCLAMATION MARK character token.
                        self.emit_char('<');
                        self.emit_char('!');
                        go!(self: to State::ScriptDataEscapeStart(Escaped));
                    },
                    // ↪ Anything else
                    _ => {
                        // Emit a U+003C LESS-THAN SIGN character token.
                        // Reconsume in the RCDATA/script data/RAWTEXT state.
                        self.emit_char('<');
                        go!(self: reconsume RawData(kind));
                    },
                }
            },

            // https://html.spec.whatwg.org/#rcdata-end-tag-open-state
            // https://html.spec.whatwg.org/#rawtext-end-tag-open-state
            // https://html.spec.whatwg.org/#script-data-end-tag-open-state
            // https://html.spec.whatwg.org/#script-data-escaped-end-tag-open-state
            states::RawEndTagOpen(kind) => loop {
                // Consume the next input character:
                let character = get_char!(self, input);

                // ↪ ASCII alpha
                if character.is_ascii_alphabetic() {
                    // Create a new end tag token, set its tag name to the empty string.
                    // Reconsume in the RCDATA end tag name state.
                    // NOTE: We don't reconsume the character but instead immediately append its lowercase version
                    // to the end tag (as that is what the new state would do).
                    let character_lowercase = character.to_ascii_lowercase();
                    go!(self: create_tag EndTag character_lowercase; push_temp character; to State::RawEndTagName(kind))
                }
                // ↪ Anything else
                else {
                    // Emit a U+003C LESS-THAN SIGN character token and a U+002F SOLIDUS character token.
                    // Reconsume in the RCDATA/RAWTEXT/script data/script data escaped state.
                    self.emit_char('<');
                    self.emit_char('/');
                    go!(self: reconsume RawData(kind));
                }
            },

            // https://html.spec.whatwg.org/#rcdata-end-tag-name-state
            // https://html.spec.whatwg.org/#rawtext-end-tag-name-state
            // https://html.spec.whatwg.org/#script-data-end-tag-name-state
            // https://html.spec.whatwg.org/#script-data-escaped-end-tag-name-state
            states::RawEndTagName(kind) => loop {
                let character = get_char!(self, input);

                // NOTE: The first three match arms in the specification are treated as "anything else" if the current
                // end tag token is NOT an appropriate end tag, so we move them into their own match block.
                if self.have_appropriate_end_tag() {
                    match character {
                        // ↪ U+0009 CHARACTER TABULATION (tab)
                        // ↪ U+000A LINE FEED (LF)
                        // ↪ U+000C FORM FEED (FF)
                        // ↪ U+0020 SPACE
                        '\t' | '\n' | '\x0C' | ' ' => {
                            // Switch to the before attribute name state
                            go!(self: clear_temp; to State::BeforeAttributeName)
                        },
                        // ↪ U+002F SOLIDUS (/)
                        '/' => {
                            // Switch to the self-closing start tag state
                            go!(self: clear_temp; to State::SelfClosingStartTag)
                        },
                        // ↪ U+003E GREATER-THAN SIGN (>)
                        '>' => {
                            // Switch to the data state and emit the current tag token
                            go!(self: clear_temp; emit_tag Data)
                        },
                        _ => {},
                    }
                }

                match lower_ascii_letter(character) {
                    // ↪ ASCII upper alpha
                    //     NOTE: This is the same as for lower alpha, but the character is lowercased first.
                    //     This is handled by lower_ascii_letter.
                    // ↪ ASCII lower alpha
                    Some(character_lowercase) => {
                        // Append the current input character to the current tag token's tag name.
                        // Append the current input character to the temporary buffer.
                        go!(self: push_tag character_lowercase; push_temp character)
                    },
                    // ↪ Anything else
                    None => {
                        // Emit a U+003C LESS-THAN SIGN character token, a U+002F SOLIDUS character token,
                        // and a character token for each of the characters in the temporary buffer
                        // (in the order they were added to the buffer).
                        // Reconsume in the RCDATA/RAWTEXT/script data/script data escaped state.
                        go!(self: discard_tag);
                        self.emit_char('<');
                        self.emit_char('/');
                        self.emit_temp_buf();
                        go!(self: reconsume RawData(kind));
                    },
                }
            },

            // https://html.spec.whatwg.org/#script-data-double-escape-start-state
            states::ScriptDataEscapeStart(DoubleEscaped) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    // ↪ U+002F SOLIDUS (/)
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    character @ ('\t' | '\n' | '\x0C' | ' ' | '/' | '>') => {
                        // If the temporary buffer is "script", then switch to the script data double escaped state.
                        // Otherwise, switch to the script data escaped state.
                        // Emit the current input character as a character token.
                        let escaped_kind = if &**self.temp_buf.borrow() == "script" {
                            DoubleEscaped
                        } else {
                            Escaped
                        };
                        self.emit_char(character);
                        go!(self: to State::RawData(ScriptDataEscaped(escaped_kind)));
                    },
                    // ↪ ASCII upper alpha
                    //     NOTE: This is the same as the "ASCII lower alpha" branch, except we lowercase the character first
                    // ↪ ASCII lower alpha
                    character => match lower_ascii_letter(character) {
                        Some(character_lowercase) => {
                            // Append the current input character to the temporary buffer.
                            // Emit the current input character as a character token.
                            go!(self: push_temp character_lowercase);
                            self.emit_char(character);
                        },
                        // ↪ Anything else
                        None => {
                            // Reconsume in the script data escaped state.
                            go!(self: reconsume RawData(ScriptDataEscaped(Escaped)))
                        },
                    },
                }
            },

            // https://html.spec.whatwg.org/#script-data-double-escaped-state
            states::RawData(ScriptDataEscaped(DoubleEscaped)) => loop {
                // Consume the next input character:
                let Some(set_result) =
                    self.pop_except_from(input, small_char_set!('\r' '\0' '-' '<' '\n'))
                else {
                    return ProcessResult::Suspend;
                };

                match set_result {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    FromSet('-') => {
                        // Switch to the script data double escaped dash state.
                        // Emit a U+002D HYPHEN-MINUS character token.
                        self.emit_char('-');
                        go!(self: to State::ScriptDataEscapedDash(DoubleEscaped));
                    },
                    // ↪ U+003C LESS-THAN SIGN (<)
                    FromSet('<') => {
                        // Switch to the script data double escaped less-than sign state.
                        // Emit a U+003C LESS-THAN SIGN character token.
                        self.emit_char('<');
                        go!(self: to State::RawLessThanSign(ScriptDataEscaped(DoubleEscaped)))
                    },
                    // ↪ U+0000 NULL
                    FromSet('\0') => {
                        // This is an unexpected-null-character parse error.
                        // Emit a U+FFFD REPLACEMENT CHARACTER character token.
                        self.bad_char_error();
                        self.emit_char('\u{fffd}');
                    },
                    // ↪ Anything else
                    //     Emit the current input character as a character token.
                    FromSet(character) => self.emit_char(character),
                    NotFromSet(characters) => self.emit_chars(characters),
                }
            },

            // https://html.spec.whatwg.org/#script-data-escape-start-state
            states::ScriptDataEscapeStart(Escaped) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => {
                        // Switch to the script data escape start dash state.
                        // Emit a U+002D HYPHEN-MINUS character token.
                        self.emit_char('-');
                        go!(self: to State::ScriptDataEscapeStartDash);
                    },
                    // ↪ Anything else
                    _ => {
                        // Reconsume in the script data state.
                        go!(self: reconsume RawData(ScriptData))
                    },
                }
            },

            // https://html.spec.whatwg.org/#script-data-escape-start-dash-state
            states::ScriptDataEscapeStartDash => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => {
                        // Switch to the script data escaped dash dash state.
                        // Emit a U+002D HYPHEN-MINUS character token.
                        self.emit_char('-');
                        go!(self: to State::ScriptDataEscapedDashDash(Escaped));
                    },
                    // ↪ Anything else
                    _ => {
                        // Reconsume in the script data state.
                        go!(self: reconsume RawData(ScriptData))
                    },
                }
            },

            // https://html.spec.whatwg.org/#script-data-escaped-state
            states::RawData(ScriptDataEscaped(Escaped)) => loop {
                // Consume the next input character:
                let Some(set_result) =
                    self.pop_except_from(input, small_char_set!('\r' '\0' '-' '<' '\n'))
                else {
                    return ProcessResult::Suspend;
                };

                match set_result {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    FromSet('-') => {
                        // Switch to the script data escaped dash state.
                        // Emit a U+002D HYPHEN-MINUS character token.
                        self.emit_char('-');
                        go!(self: to State::ScriptDataEscapedDash(Escaped));
                    },
                    // ↪ U+003C LESS-THAN SIGN (<)
                    FromSet('<') => {
                        // Switch to the script data escaped less-than sign state.
                        go!(self: to State::RawLessThanSign(ScriptDataEscaped(Escaped)))
                    },
                    // ↪ U+0000 NULL
                    FromSet('\0') => {
                        // This is an unexpected-null-character parse error.
                        // Emit a U+FFFD REPLACEMENT CHARACTER character token.
                        self.bad_char_error();
                        self.emit_char('\u{fffd}');
                    },
                    // ↪ Anything else
                    //     Emit the current input character as a character token.
                    FromSet(character) => self.emit_char(character),
                    NotFromSet(characters) => self.emit_chars(characters),
                }
            },

            // https://html.spec.whatwg.org/#script-data-escaped-dash-state
            // https://html.spec.whatwg.org/#script-data-double-escaped-dash-state
            states::ScriptDataEscapedDash(kind) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => {
                        // Switch to the script data escaped dash dash/script data double escaped dash dash state.
                        // Emit a U+002D HYPHEN-MINUS character token.
                        self.emit_char('-');
                        go!(self: to State::ScriptDataEscapedDashDash(kind));
                    },
                    // ↪ U+003C LESS-THAN SIGN (<)
                    '<' => {
                        // Switch to the script data escaped less-than sign/script data double escaped less-than sign state.
                        if kind == DoubleEscaped {
                            self.emit_char('<');
                        }
                        go!(self: to State::RawLessThanSign(ScriptDataEscaped(kind)));
                    },
                    // ↪ U+0000 NULL
                    '\0' => {
                        // This is an unexpected-null-character parse error.
                        // Switch to the script data escaped/script data double escaped state.
                        // Emit a U+FFFD REPLACEMENT CHARACTER character token.
                        self.bad_char_error();
                        self.emit_char('\u{fffd}');
                        go!(self: to State::RawData(ScriptDataEscaped(kind)));
                    },
                    // ↪ Anything else
                    c => {
                        // Switch to the script data escaped/script data double escaped state.
                        // Emit the current input character as a character token.
                        self.emit_char(c);
                        go!(self: to State::RawData(ScriptDataEscaped(kind)));
                    },
                }
            },

            // https://html.spec.whatwg.org/#script-data-escaped-dash-dash-state
            // https://html.spec.whatwg.org/#script-data-double-escaped-dash-dash-state
            states::ScriptDataEscapedDashDash(kind) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => {
                        // Emit a U+002D HYPHEN-MINUS character token.
                        self.emit_char('-');
                    },
                    // ↪ U+003C LESS-THAN SIGN (<)
                    '<' => {
                        // Switch to the script data escaped less-than sign/script data double escaped less-than sign state.
                        if kind == DoubleEscaped {
                            self.emit_char('<');
                        }
                        go!(self: to State::RawLessThanSign(ScriptDataEscaped(kind)));
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the script data state.
                        // Emit a U+003E GREATER-THAN SIGN character token.
                        self.emit_char('>');
                        go!(self: to State::RawData(ScriptData));
                    },
                    // ↪ U+0000 NULL
                    '\0' => {
                        // This is an unexpected-null-character parse error.
                        // Switch to the script data escaped/script data double escaped state.
                        // Emit a U+FFFD REPLACEMENT CHARACTER character token.
                        self.bad_char_error();
                        self.emit_char('\u{fffd}');
                        go!(self: to State::RawData(ScriptDataEscaped(kind)))
                    },
                    // ↪ Anything else
                    character => {
                        // Switch to the script data escaped/script data double escaped state.
                        // Emit the current input character as a character token.
                        self.emit_char(character);
                        go!(self: to State::RawData(ScriptDataEscaped(kind)));
                    },
                }
            },

            // https://html.spec.whatwg.org/#script-data-double-escape-end-state
            states::ScriptDataDoubleEscapeEnd => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    // ↪ U+002F SOLIDUS (/)
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    character @ ('\t' | '\n' | '\x0C' | ' ' | '/' | '>') => {
                        // If the temporary buffer is "script", then switch to the script data escaped state.
                        // Otherwise, switch to the script data double escaped state.
                        // Emit the current input character as a character token.
                        let escaped_kind = if &**self.temp_buf.borrow() == "script" {
                            Escaped
                        } else {
                            DoubleEscaped
                        };
                        self.emit_char(character);
                        go!(self: to State::RawData(ScriptDataEscaped(escaped_kind)));
                    },

                    character => match lower_ascii_letter(character) {
                        // ↪ ASCII upper alpha
                        //     NOTE: This is the same as "ASCII lower alpha", except we lowercase the character first.
                        //     That is handled by lower_ascii_letter.
                        // ↪ ASCII lower alpha
                        Some(character_lowercase) => {
                            // Append the current input character to the temporary buffer.
                            // Emit the current input character as a character token.
                            go!(self: push_temp character_lowercase);
                            self.emit_char(character);
                        },
                        // ↪ Anything else
                        None => {
                            // Reconsume in the script data double escaped state.
                            go!(self: reconsume RawData(ScriptDataEscaped(DoubleEscaped)))
                        },
                    },
                }
            },

            // https://html.spec.whatwg.org/#before-attribute-name-state
            states::BeforeAttributeName => loop {
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Ignore the character.
                    },
                    // U+002F SOLIDUS (/)
                    '/' => {
                        // Reconsume in the after attribute name state.
                        // NOTE: Instead we move to the self closing start tag,
                        // as that is what the "after attribute name" state would do.
                        go!(self: to State::SelfClosingStartTag)
                    },
                    // U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Reconsume in the after attribute name state.
                        // NOTE: Instead we emit the current tag and move to the data state,
                        // as that is what the "after attribute name" state would do.
                        go!(self: emit_tag Data)
                    },
                    // NOTE: In the "anything else" case we should reconsume in the attribute name state,
                    // but instead of reconsuming we inline what that state *would* do here.
                    '\0' => {
                        self.bad_char_error();
                        go!(self: create_attr '\u{fffd}'; to State::AttributeName)
                    },
                    character => match lower_ascii_letter(character) {
                        Some(character) => {
                            go!(self: create_attr character; to State::AttributeName)
                        },
                        None => {
                            if matches!(character, '"' | '\'' | '<' | '=') {
                                self.bad_char_error();
                            }

                            go!(self: create_attr character; to State::AttributeName);
                        },
                    },
                }
            },

            // https://html.spec.whatwg.org/#attribute-name-state
            states::AttributeName => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Reconsume in the after attribute name state.
                        // NOTE: Instead we move to the after attribute name state and ignore
                        // the character, as that state would ignore it as well.
                        go!(self: to State::AfterAttributeName)
                    },
                    // ↪ U+002F SOLIDUS (/)
                    '/' => {
                        // Reconsume in the after attribute name state.
                        // NOTE: Instead we move to the self closing start tag state, as that is
                        // what the after attribute name state would do when it encounters a '/'.
                        go!(self: to State::SelfClosingStartTag)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Reconsume in the after attribute name state.
                        // NOTE: Instead we move to the data state, as that is
                        // what the after attribute name state would do when it encounters a '>'.
                        go!(self: emit_tag Data)
                    },
                    // ↪ U+003D EQUALS SIGN (=)
                    '=' => {
                        // Switch to the before attribute value state.
                        go!(self: to State::BeforeAttributeValue)
                    },
                    // ↪ U+0000 NULL
                    '\0' => {
                        // This is an unexpected-null-character parse error.
                        // Append a U+FFFD REPLACEMENT CHARACTER character to the current attribute's name.
                        self.bad_char_error();
                        go!(self: push_name '\u{fffd}')
                    },
                    character => match lower_ascii_letter(character) {
                        // ↪ ASCII upper alpha
                        Some(character_lowercase) => {
                            // Append the lowercase version of the current input character
                            // (add 0x0020 to the character's code point) to the current attribute's name.
                            go!(self: push_name character_lowercase)
                        },
                        None => {
                            // ↪ U+0022 QUOTATION MARK (")
                            // ↪ U+0027 APOSTROPHE (')
                            // ↪ U+003C LESS-THAN SIGN (<)
                            if matches!(character, '"' | '\'' | '<') {
                                // This is an unexpected-character-in-attribute-name parse error.
                                // Treat it as per the "anything else" entry below.
                                self.bad_char_error();
                            }
                            // ↪ Anything else
                            // Append the current input character to the current attribute's name.
                            go!(self: push_name character);
                        },
                    },
                }
            },

            // https://html.spec.whatwg.org/#after-attribute-name-state
            states::AfterAttributeName => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Ignore the character.
                    },
                    // ↪ U+002F SOLIDUS (/)
                    '/' => {
                        // Switch to the self-closing start tag state.
                        go!(self: to State::SelfClosingStartTag)
                    },
                    // ↪ U+003D EQUALS SIGN (=)
                    '=' => {
                        // Switch to the before attribute value state.
                        go!(self: to State::BeforeAttributeValue)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the data state. Emit the current tag token.
                        go!(self: emit_tag Data)
                    },
                    // NOTE: The "anything else" match arm in the specification reconsumes
                    // the input in the attribute name state. Instead of reconsuming we inline
                    // what the attribute name state *would* do, and the move to it.
                    '\0' => {
                        self.bad_char_error();
                        go!(self: create_attr '\u{fffd}'; to State::AttributeName)
                    },
                    character => match lower_ascii_letter(character) {
                        Some(character_lowercase) => {
                            go!(self: create_attr character_lowercase; to State::AttributeName)
                        },
                        None => {
                            if matches!(character, '"' | '\'' | '<') {
                                self.bad_char_error();
                            }

                            go!(self: create_attr character; to State::AttributeName);
                        },
                    },
                }
            },

            // https://html.spec.whatwg.org/#before-attribute-value-state
            // Use peek so we can handle the first attr character along with the rest,
            // hopefully in the same zero-copy buffer.
            states::BeforeAttributeValue => loop {
                // Consume the next input character:
                match peek!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\r' | '\x0C' | ' ' => {
                        // Ignore the character.
                        go!(self: discard_char input)
                    },
                    // ↪ U+0022 QUOTATION MARK (")
                    '"' => {
                        // Switch to the attribute value (double-quoted) state.
                        go!(self: discard_char input; to State::AttributeValue(DoubleQuoted))
                    },
                    // ↪ U+0027 APOSTROPHE (')
                    '\'' => {
                        // Switch to the attribute value (single-quoted) state.
                        go!(self: discard_char input; to State::AttributeValue(SingleQuoted))
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // This is a missing-attribute-value parse error.
                        // Switch to the data state.
                        // Emit the current tag token.
                        go!(self: discard_char input);
                        self.bad_char_error();
                        go!(self: emit_tag Data)
                    },
                    // ↪ Anything else
                    _ => {
                        // Reconsume in the attribute value (unquoted) state.
                        go!(self: to State::AttributeValue(Unquoted))
                    },
                }
            },

            // https://html.spec.whatwg.org/#attribute-value-(double-quoted)-state
            states::AttributeValue(DoubleQuoted) => loop {
                // Consume the next input character:
                let Some(set_result) =
                    self.pop_except_from(input, small_char_set!('\r' '"' '&' '\0' '\n'))
                else {
                    return ProcessResult::Suspend;
                };

                match set_result {
                    // ↪ U+0022 QUOTATION MARK (")
                    FromSet('"') => {
                        // Switch to the after attribute value (quoted) state.
                        go!(self: to State::AfterAttributeValueQuoted)
                    },
                    // ↪ U+0026 AMPERSAND (&)
                    FromSet('&') => {
                        // Set the return state to the attribute value (double-quoted) state.
                        // Switch to the character reference state.
                        go!(self: consume_char_ref)
                    },
                    // ↪ U+0000 NULL
                    FromSet('\0') => {
                        // This is an unexpected-null-character parse error.
                        // Append a U+FFFD REPLACEMENT CHARACTER character to the current attribute's value.
                        self.bad_char_error();
                        go!(self: push_value '\u{fffd}')
                    },
                    // ↪ Anything else
                    //     Append the current input character to the current attribute's value.
                    FromSet(character) => go!(self: push_value character),
                    NotFromSet(ref characters) => go!(self: append_value characters),
                }
            },

            // https://html.spec.whatwg.org/#attribute-value-(single-quoted)-state
            states::AttributeValue(SingleQuoted) => loop {
                // Consume the next input character:
                let Some(set_result) =
                    self.pop_except_from(input, small_char_set!('\r' '\'' '&' '\0' '\n'))
                else {
                    return ProcessResult::Suspend;
                };

                match set_result {
                    // ↪ U+0027 APOSTROPHE (')
                    FromSet('\'') => {
                        // Switch to the after attribute value (quoted) state.
                        go!(self: to State::AfterAttributeValueQuoted)
                    },
                    // ↪ U+0026 AMPERSAND (&)
                    FromSet('&') => {
                        // Set the return state to the attribute value (single-quoted) state.
                        // Switch to the character reference state.
                        go!(self: consume_char_ref)
                    },
                    // ↪ U+0000 NULL
                    FromSet('\0') => {
                        // This is an unexpected-null-character parse error.
                        // Append a U+FFFD REPLACEMENT CHARACTER character to the current attribute's value.
                        self.bad_char_error();
                        go!(self: push_value '\u{fffd}')
                    },
                    // ↪ Anything else
                    //     Append the current input character to the current attribute's value.
                    FromSet(character) => go!(self: push_value character),
                    NotFromSet(ref characters) => go!(self: append_value characters),
                }
            },

            // https://html.spec.whatwg.org/#attribute-value-(unquoted)-state
            states::AttributeValue(Unquoted) => loop {
                // Consume the next input character:
                let Some(set_result) = self.pop_except_from(
                    input,
                    small_char_set!('\r' '\t' '\n' '\x0C' ' ' '&' '>' '\0'),
                ) else {
                    return ProcessResult::Suspend;
                };

                match set_result {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    FromSet('\t') | FromSet('\n') | FromSet('\x0C') | FromSet(' ') => {
                        // Switch to the before attribute name state.
                        go!(self: to State::BeforeAttributeName)
                    },
                    // ↪ U+0026 AMPERSAND (&)
                    FromSet('&') => {
                        // Set the return state to the attribute value (unquoted) state.
                        // Switch to the character reference state.
                        go!(self: consume_char_ref)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    FromSet('>') => {
                        // Switch to the data state.
                        // Emit the current tag token.
                        go!(self: emit_tag Data)
                    },
                    // ↪ U+0000 NULL
                    FromSet('\0') => {
                        // This is an unexpected-null-character parse error.
                        // Append a U+FFFD REPLACEMENT CHARACTER character to the current attribute's value.
                        self.bad_char_error();
                        go!(self: push_value '\u{fffd}')
                    },
                    FromSet(c) => {
                        // ↪ U+0022 QUOTATION MARK (")
                        // ↪ U+0027 APOSTROPHE (')
                        // ↪ U+003C LESS-THAN SIGN (<)
                        // ↪ U+003D EQUALS SIGN (=)
                        // ↪ U+0060 GRAVE ACCENT (`)
                        if matches!(c, '"' | '\'' | '<' | '=' | '`') {
                            // This is an unexpected-character-in-unquoted-attribute-value parse error.
                            // Treat it as per the "anything else" entry below.
                            self.bad_char_error();
                        }
                        // ↪ Anything else
                        //     Append the current input character to the current attribute's value.
                        go!(self: push_value c);
                    },
                    // ↪ Anything else
                    NotFromSet(ref characters) => {
                        // Append the current input character to the current attribute's value.
                        go!(self: append_value characters)
                    },
                }
            },

            // https://html.spec.whatwg.org/#after-attribute-value-(quoted)-state
            states::AfterAttributeValueQuoted => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Switch to the before attribute name state.
                        go!(self: to State::BeforeAttributeName)
                    },
                    // ↪ U+002F SOLIDUS (/)
                    '/' => {
                        // Switch to the self-closing start tag state.
                        go!(self: to State::SelfClosingStartTag)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the data state. Emit the current tag token.
                        go!(self: emit_tag Data)
                    },
                    // ↪ Anything else
                    _ => {
                        // This is a missing-whitespace-between-attributes parse error.
                        // Reconsume in the before attribute name state.
                        self.bad_char_error();
                        go!(self: reconsume BeforeAttributeName)
                    },
                }
            },

            // https://html.spec.whatwg.org/#self-closing-start-tag-state
            states::SelfClosingStartTag => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Set the self-closing flag of the current tag token.
                        // Switch to the data state. Emit the current tag token.
                        self.current_tag_self_closing.set(true);
                        go!(self: emit_tag Data);
                    },
                    // ↪ Anything else
                    _ => {
                        // This is an unexpected-solidus-in-tag parse error.
                        // Reconsume in the before attribute name state.
                        self.bad_char_error();
                        go!(self: reconsume BeforeAttributeName)
                    },
                }
            },

            //§ bogus-comment-state
            states::BogusComment => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the data state. Emit the current comment token.
                        go!(self: emit_comment; to State::Data)
                    },
                    // ↪ U+0000 NULL
                    '\0' => {
                        // This is an unexpected-null-character parse error.
                        // Append a U+FFFD REPLACEMENT CHARACTER character to the comment token's data.
                        self.bad_char_error();
                        go!(self: push_comment '\u{fffd}')
                    },
                    // ↪ Anything else
                    character => {
                        // Append the current input character to the comment token's data.
                        go!(self: push_comment character)
                    },
                }
            },

            // https://html.spec.whatwg.org/#markup-declaration-open-state
            states::MarkupDeclarationOpen => loop {
                // If the next few characters are:
                // ↪ Two U+002D HYPHEN-MINUS characters (-)
                if eat_exact!(self, input, "--") {
                    go!(self: clear_comment; to State::CommentStart);
                }
                // ↪ ASCII case-insensitive match for "DOCTYPE"
                else if eat!(self, input, "doctype") {
                    go!(self: to State::Doctype);
                } else {
                    // ↪ "[CDATA["
                    if self
                        .sink
                        .adjusted_current_node_present_but_not_in_html_namespace()
                        && eat_exact!(self, input, "[CDATA[")
                    {
                        // Consume those characters.
                        // If there is an adjusted current node and it is not an element in the HTML namespace,
                        // then switch to the CDATA section state. Otherwise, this is a cdata-in-html-content parse error.
                        // Create a comment token whose data is "[CDATA[". Switch to the bogus comment state.
                        // FIXME: Create that comment token.
                        go!(self: clear_temp; to State::CdataSection);
                    }

                    // ↪ Anything else
                    //     This is an incorrectly-opened-comment parse error.
                    //     Create a comment token whose data is the empty string.
                    //     Switch to the bogus comment state (don't consume anything in the current state).
                    // FIXME: Create that comment token.
                    self.bad_char_error();
                    go!(self: clear_comment; to State::BogusComment);
                }
            },

            // https://html.spec.whatwg.org/#comment-start-state
            states::CommentStart => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => {
                        // Switch to the comment start dash state.
                        go!(self: to State::CommentStartDash)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // This is an abrupt-closing-of-empty-comment parse error.
                        // Switch to the data state.
                        // Emit the current comment token.
                        self.bad_char_error();
                        go!(self: emit_comment; to State::Data)
                    },
                    // NOTE: The "anything else" case in the specification reconsumes the character in the
                    // comment state, instead we inline what the comment state *would* do if it encountered
                    // that character.
                    '\0' => {
                        self.bad_char_error();
                        go!(self: push_comment '\u{fffd}'; to State::Comment)
                    },
                    character => go!(self: push_comment character; to State::Comment),
                }
            },

            // https://html.spec.whatwg.org/#comment-start-dash-state
            states::CommentStartDash => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => {
                        // Switch to the comment end state.
                        go!(self: to State::CommentEnd)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // This is an abrupt-closing-of-empty-comment parse error.
                        // Switch to the data state.
                        // Emit the current comment token.
                        self.bad_char_error();
                        go!(self: emit_comment; to State::Data)
                    },
                    // NOTE: The "anything else" case in the specification reconsumes the character in the
                    // comment state, instead we inline what the comment state *would* do if it encountered
                    // that character.
                    '\0' => {
                        self.bad_char_error();
                        go!(self: append_comment "-\u{fffd}"; to State::Comment)
                    },
                    character => {
                        go!(self: push_comment '-'; push_comment character; to State::Comment)
                    },
                }
            },

            // https://html.spec.whatwg.org/#comment-state
            states::Comment => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+003C LESS-THAN SIGN (<)
                    c @ '<' => {
                        // Append the current input character to the comment token's data.
                        // Switch to the comment less-than sign state.
                        go!(self: push_comment c; to State::CommentLessThanSign)
                    },
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => {
                        // Switch to the comment end dash state.
                        go!(self: to State::CommentEndDash)
                    },
                    // ↪ U+0000 NULL
                    '\0' => {
                        // This is an unexpected-null-character parse error.
                        // Append a U+FFFD REPLACEMENT CHARACTER character to the comment token's data.
                        self.bad_char_error();
                        go!(self: push_comment '\u{fffd}')
                    },
                    // ↪ Anything else
                    character => {
                        // Append the current input character to the comment token's data.
                        go!(self: push_comment character)
                    },
                }
            },

            // https://html.spec.whatwg.org/#comment-less-than-sign-state
            states::CommentLessThanSign => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0021 EXCLAMATION MARK (!)
                    c @ '!' => {
                        // Append the current input character to the comment token's data.
                        // Switch to the comment less-than sign bang state.
                        go!(self: push_comment c; to State::CommentLessThanSignBang)
                    },
                    // ↪ U+003C LESS-THAN SIGN (<)
                    c @ '<' => {
                        // Append the current input character to the comment token's data.
                        go!(self: push_comment c)
                    },
                    // ↪ Anything else
                    _ => {
                        // Reconsume in the comment state.
                        go!(self: reconsume Comment)
                    },
                }
            },

            // https://html.spec.whatwg.org/#comment-less-than-sign-bang-state
            states::CommentLessThanSignBang => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => {
                        // Switch to the comment less-than sign bang dash state.
                        go!(self: to State::CommentLessThanSignBangDash)
                    },
                    // ↪ Anything else
                    _ => {
                        // Reconsume in the comment state.
                        go!(self: reconsume Comment)
                    },
                }
            },

            // https://html.spec.whatwg.org/#comment-less-than-sign-bang-dash-state
            states::CommentLessThanSignBangDash => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => {
                        // Switch to the comment less-than sign bang dash dash state.
                        go!(self: to State::CommentLessThanSignBangDashDash)
                    },
                    // ↪ Anything else
                    _ => {
                        // Reconsume in the comment end dash state.
                        go!(self: reconsume CommentEndDash)
                    },
                }
            },

            // https://html.spec.whatwg.org/#comment-less-than-sign-bang-dash-dash-state
            states::CommentLessThanSignBangDashDash => loop {
                match get_char!(self, input) {
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Reconsume in the comment end state.
                        go!(self: reconsume CommentEnd)
                    },
                    // ↪ Anything else
                    _ => {
                        // This is a nested-comment parse error.
                        // Reconsume in the comment end state.
                        self.bad_char_error();
                        go!(self: reconsume CommentEnd)
                    },
                }
            },

            // https://html.spec.whatwg.org/#comment-end-dash-state
            states::CommentEndDash => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => {
                        // Switch to the comment end state.
                        go!(self: to State::CommentEnd)
                    },
                    // NOTE: The "anything else" case in the specification reconsumes the character in the
                    // comment state. Instead we inline what the comment state *would* do here.
                    '\0' => {
                        self.bad_char_error();
                        go!(self: append_comment "-\u{fffd}"; to State::Comment)
                    },
                    character => {
                        go!(self: push_comment '-'; push_comment character; to State::Comment)
                    },
                }
            },

            // https://html.spec.whatwg.org/#comment-end-state
            states::CommentEnd => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the data state.
                        // Emit the current comment token.
                        go!(self: emit_comment; to State::Data)
                    },
                    // ↪ U+0021 EXCLAMATION MARK (!)
                    '!' => {
                        // Switch to the comment end bang state.
                        go!(self: to State::CommentEndBang)
                    },
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => {
                        // Append a U+002D HYPHEN-MINUS character (-) to the comment token's data.
                        go!(self: push_comment '-')
                    },
                    // ↪ Anything else
                    _ => {
                        // Append two U+002D HYPHEN-MINUS characters (-) to the comment token's data.
                        // Reconsume in the comment state.
                        go!(self: append_comment "--"; reconsume Comment)
                    },
                }
            },

            // https://html.spec.whatwg.org/#comment-end-bang-state
            states::CommentEndBang => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+002D HYPHEN-MINUS (-)
                    '-' => go!(self: append_comment "--!"; to State::CommentEndDash),
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        self.bad_char_error();
                        go!(self: emit_comment; to State::Data)
                    },
                    // NOTE: The "anything else" case in the specification reconsumes the character in the
                    // comment state. Instead we inline what the comment state *would* do here.
                    '\0' => {
                        self.bad_char_error();
                        go!(self: append_comment "--!\u{fffd}"; to State::Comment)
                    },
                    character => {
                        go!(self: append_comment "--!"; push_comment character; to State::Comment)
                    },
                }
            },

            // https://html.spec.whatwg.org/#doctype-state
            states::Doctype => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Switch to the before DOCTYPE name state.
                        go!(self: to State::BeforeDoctypeName)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Reconsume in the before DOCTYPE name state.
                        go!(self: reconsume BeforeDoctypeName)
                    },
                    // ↪ Anything else
                    _ => {
                        // This is a missing-whitespace-before-doctype-name parse error.
                        // Reconsume in the before DOCTYPE name state.
                        self.bad_char_error();
                        go!(self: reconsume BeforeDoctypeName)
                    },
                }
            },

            // https://html.spec.whatwg.org/#before-doctype-name-state
            states::BeforeDoctypeName => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Ignore the character.
                    },
                    // ↪ U+0000 NULL
                    '\0' => {
                        // This is an unexpected-null-character parse error.
                        // Create a new DOCTYPE token.
                        // Set the token's name to a U+FFFD REPLACEMENT CHARACTER character.
                        // Switch to the DOCTYPE name state.
                        self.bad_char_error();
                        go!(self: create_doctype; push_doctype_name '\u{fffd}'; to State::DoctypeName)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // This is a missing-doctype-name parse error.
                        // Create a new DOCTYPE token.
                        // Set its force-quirks flag to on.
                        // Switch to the data state.
                        // Emit the current token.
                        self.bad_char_error();
                        go!(self: create_doctype; force_quirks; emit_doctype; to State::Data)
                    },
                    // ↪ ASCII upper alpha
                    //     NOTE: This is the same as "anythign else", except we use the lowercase version of the character.
                    // ↪ Anything else
                    character => {
                        // Create a new DOCTYPE token.
                        // Set the token's name to the current input character.
                        // Switch to the DOCTYPE name state.
                        go!(self: create_doctype; push_doctype_name (character.to_ascii_lowercase());
                                  to State::DoctypeName)
                    },
                }
            },

            // https://html.spec.whatwg.org/#doctype-name-state
            states::DoctypeName => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Switch to the after DOCTYPE name state.
                        go!(self: clear_temp; to State::AfterDoctypeName)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the data state. Emit the current DOCTYPE token.
                        go!(self: emit_doctype; to State::Data)
                    },
                    // ↪ U+0000 NULL
                    '\0' => {
                        // This is an unexpected-null-character parse error.
                        // Append a U+FFFD REPLACEMENT CHARACTER character to the current DOCTYPE token's name.
                        self.bad_char_error();
                        go!(self: push_doctype_name '\u{fffd}')
                    },
                    // ↪ ASCII upper alpha
                    //     NOTE: This is the same as "anythign else", except we use the lowercase version of the character.
                    // ↪ Anything else
                    character => {
                        // Append the current input character to the current DOCTYPE token's name.
                        go!(self: push_doctype_name (character.to_ascii_lowercase()))
                    },
                }
            },

            // https://html.spec.whatwg.org/#after-doctype-name-state
            states::AfterDoctypeName => loop {
                // NOTE: We move some steps out of the "anything else" case to the front for convenience.

                // If the six characters starting from the current input character are an
                // ASCII case-insensitive match for "PUBLIC", then consume those characters
                // and switch to the after DOCTYPE public keyword state.
                if eat!(self, input, "public") {
                    go!(self: to State::AfterDoctypeKeyword(Public));
                }
                // Otherwise, if the six characters starting from the current input character
                // are an ASCII case-insensitive match for "SYSTEM", then consume those characters
                // and switch to the after DOCTYPE system keyword state.
                else if eat!(self, input, "system") {
                    go!(self: to State::AfterDoctypeKeyword(System));
                } else {
                    // Consume the next input character:
                    match get_char!(self, input) {
                        // ↪ U+0009 CHARACTER TABULATION (tab)
                        // ↪ U+000A LINE FEED (LF)
                        // ↪ U+000C FORM FEED (FF)
                        // ↪ U+0020 SPACE
                        '\t' | '\n' | '\x0C' | ' ' => {
                            // Ignore the character.
                        },
                        // ↪ U+003E GREATER-THAN SIGN (>)
                        '>' => {
                            // Switch to the data state. Emit the current DOCTYPE token.
                            go!(self: emit_doctype; to State::Data)
                        },
                        // ↪ Anything else
                        _ => {
                            // Otherwise, this is an invalid-character-sequence-after-doctype-name parse error.
                            // Set the current DOCTYPE token's force-quirks flag to on.
                            // Reconsume in the bogus DOCTYPE state.
                            self.bad_char_error();
                            go!(self: force_quirks; reconsume BogusDoctype)
                        },
                    }
                }
            },

            // https://html.spec.whatwg.org/#after-doctype-public-keyword-state
            // https://html.spec.whatwg.org/#after-doctype-system-keyword-state
            states::AfterDoctypeKeyword(kind) => loop {
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Switch to the before DOCTYPE public/system identifier state.
                        go!(self: to State::BeforeDoctypeIdentifier(kind))
                    },
                    // ↪ U+0022 QUOTATION MARK (")
                    '"' => {
                        // This is a missing-whitespace-after-doctype-public/system-keyword parse error.
                        // Set the current DOCTYPE token's public/system identifier to the empty string
                        // (not missing), then switch to the DOCTYPE public/system identifier (double-quoted)
                        // state.
                        self.bad_char_error();
                        go!(self: clear_doctype_id kind; to State::DoctypeIdentifierDoubleQuoted(kind))
                    },
                    // ↪ U+0027 APOSTROPHE (')
                    '\'' => {
                        // This is a missing-whitespace-after-doctype-public-keyword parse error.
                        // Set the current DOCTYPE token's public identifier to the empty string
                        // (not missing), then switch to the DOCTYPE public/system identifier (single-quoted)
                        // state.
                        self.bad_char_error();
                        go!(self: clear_doctype_id kind; to State::DoctypeIdentifierSingleQuoted(kind))
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // This is a missing-doctype-public-identifier parse error.
                        // Set the current DOCTYPE token's force-quirks flag to on.
                        // Switch to the data state.
                        // Emit the current DOCTYPE token.
                        self.bad_char_error();
                        go!(self: force_quirks; emit_doctype; to State::Data)
                    },
                    // ↪ Anything else
                    _ => {
                        // This is a missing-quote-before-doctype-public-identifier parse error.
                        // Set the current DOCTYPE token's force-quirks flag to on.
                        // Reconsume in the bogus DOCTYPE state.
                        self.bad_char_error();
                        go!(self: force_quirks; reconsume BogusDoctype)
                    },
                }
            },

            // https://html.spec.whatwg.org/#before-doctype-public-identifier-state
            // https://html.spec.whatwg.org/#before-doctype-system-identifier-state
            states::BeforeDoctypeIdentifier(kind) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Ignore the character.
                    },
                    // ↪ U+0022 QUOTATION MARK (")
                    '"' => {
                        // Set the current DOCTYPE token's public/system identifier to the empty string
                        // (not missing), then switch to the DOCTYPE public/system identifier (double-quoted)
                        // state.
                        go!(self: clear_doctype_id kind; to State::DoctypeIdentifierDoubleQuoted(kind))
                    },
                    // ↪ U+0027 APOSTROPHE (')
                    '\'' => {
                        // Set the current DOCTYPE token's public/systen identifier to the empty string
                        // (not missing), then switch to the DOCTYPE public/system identifier (single-quoted)
                        // state.
                        go!(self: clear_doctype_id kind; to State::DoctypeIdentifierSingleQuoted(kind))
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // This is a missing-doctype-public/system-identifier parse error.
                        // Set the current DOCTYPE token's force-quirks flag to on.
                        // Switch to the data state.
                        // Emit the current DOCTYPE token.
                        self.bad_char_error();
                        go!(self: force_quirks; emit_doctype; to State::Data)
                    },
                    // ↪ Anything else
                    _ => {
                        // This is a missing-quote-before-doctype-public/system-identifier parse error.
                        // Set the current DOCTYPE token's force-quirks flag to on.
                        // Reconsume in the bogus DOCTYPE state.
                        self.bad_char_error();
                        go!(self: force_quirks; reconsume BogusDoctype)
                    },
                }
            },

            // https://html.spec.whatwg.org/#doctype-public-identifier-(double-quoted)-state
            // https://html.spec.whatwg.org/#doctype-system-identifier-(double-quoted)-state
            states::DoctypeIdentifierDoubleQuoted(kind) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0022 QUOTATION MARK (")
                    '"' => {
                        // Switch to the after DOCTYPE public/system identifier state.
                        go!(self: to State::AfterDoctypeIdentifier(kind))
                    },
                    // ↪ U+0000 NULL
                    '\0' => {
                        // This is an unexpected-null-character parse error.
                        // Append a U+FFFD REPLACEMENT CHARACTER character to the current
                        // DOCTYPE token's public identifier.
                        self.bad_char_error();
                        go!(self: push_doctype_id kind '\u{fffd}')
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // This is an abrupt-doctype-public-identifier parse error.
                        // Set the current DOCTYPE token's force-quirks flag to on.
                        // Switch to the data state.
                        // Emit the current DOCTYPE token.
                        self.bad_char_error();
                        go!(self: force_quirks; emit_doctype; to State::Data)
                    },
                    // ↪ Anything else
                    character => go!(self: push_doctype_id kind character),
                }
            },

            // https://html.spec.whatwg.org/#doctype-public-identifier-(single-quoted)-state
            // https://html.spec.whatwg.org/#doctype-system-identifier-(single-quoted)-state
            states::DoctypeIdentifierSingleQuoted(kind) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0027 APOSTROPHE (')
                    '\'' => {
                        // Switch to the after DOCTYPE public/system identifier state.
                        go!(self: to State::AfterDoctypeIdentifier(kind))
                    },
                    // ↪ U+0000 NULL
                    '\0' => {
                        // This is an unexpected-null-character parse error.
                        // Append a U+FFFD REPLACEMENT CHARACTER character
                        // to the current DOCTYPE token's public/system identifier.
                        self.bad_char_error();
                        go!(self: push_doctype_id kind '\u{fffd}')
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // This is an abrupt-doctype-public/system-identifier parse error.
                        // Set the current DOCTYPE token's force-quirks flag to on.
                        // Switch to the data state.
                        // Emit the current DOCTYPE token.
                        self.bad_char_error();
                        go!(self: force_quirks; emit_doctype; to State::Data)
                    },
                    // ↪ Anything else
                    character => {
                        // Append the current input character to the current DOCTYPE token's
                        // public/system identifier.
                        go!(self: push_doctype_id kind character)
                    },
                }
            },

            // https://html.spec.whatwg.org/#after-doctype-public-identifier-state
            states::AfterDoctypeIdentifier(Public) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Switch to the between DOCTYPE public and system identifiers state.
                        go!(self: to State::BetweenDoctypePublicAndSystemIdentifiers)
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the data state. Emit the current DOCTYPE token.
                        go!(self: emit_doctype; to State::Data)
                    },
                    // ↪ U+0022 QUOTATION MARK (")
                    '"' => {
                        // This is a missing-whitespace-between-doctype-public-and-system-identifiers
                        // parse error. Set the current DOCTYPE token's system identifier to the empty string
                        // (not missing), then switch to the DOCTYPE system identifier (double-quoted) state.
                        self.bad_char_error();
                        go!(self: clear_doctype_id System; to State::DoctypeIdentifierDoubleQuoted(System))
                    },
                    // ↪ U+0027 APOSTROPHE (')
                    '\'' => {
                        // This is a missing-whitespace-between-doctype-public-and-system-identifiers parse error.
                        // Set the current DOCTYPE token's system identifier to the empty string (not missing),
                        // then switch to the DOCTYPE system identifier (single-quoted) state.
                        self.bad_char_error();
                        go!(self: clear_doctype_id System; to State::DoctypeIdentifierSingleQuoted(System))
                    },
                    // ↪ Anything else
                    _ => {
                        // This is a missing-quote-before-doctype-system-identifier parse error.
                        // Set the current DOCTYPE token's force-quirks flag to on.
                        // Reconsume in the bogus DOCTYPE state.
                        self.bad_char_error();
                        go!(self: force_quirks; reconsume BogusDoctype)
                    },
                }
            },

            // https://html.spec.whatwg.org/#between-doctype-public-and-system-identifiers-state
            states::BetweenDoctypePublicAndSystemIdentifiers => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Ignore the character.
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the data state. Emit the current DOCTYPE token.
                        go!(self: emit_doctype; to State::Data)
                    },
                    // ↪ U+0022 QUOTATION MARK (")
                    '"' => {
                        // Set the current DOCTYPE token's system identifier to the empty string (not missing),
                        // then switch to the DOCTYPE system identifier (double-quoted) state.
                        go!(self: clear_doctype_id System; to State::DoctypeIdentifierDoubleQuoted(System))
                    },
                    // ↪ U+0027 APOSTROPHE (')
                    '\'' => {
                        // Set the current DOCTYPE token's system identifier to the empty string (not missing),
                        // then switch to the DOCTYPE system identifier (single-quoted) state.
                        go!(self: clear_doctype_id System; to State::DoctypeIdentifierSingleQuoted(System))
                    },
                    // ↪ Anything else
                    _ => {
                        // This is a missing-quote-before-doctype-system-identifier parse error.
                        // Set the current DOCTYPE token's force-quirks flag to on.
                        // Reconsume in the bogus DOCTYPE state.
                        self.bad_char_error();
                        go!(self: force_quirks; reconsume BogusDoctype)
                    },
                }
            },

            // https://html.spec.whatwg.org/#after-doctype-system-identifier-state
            states::AfterDoctypeIdentifier(System) => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+0009 CHARACTER TABULATION (tab)
                    // ↪ U+000A LINE FEED (LF)
                    // ↪ U+000C FORM FEED (FF)
                    // ↪ U+0020 SPACE
                    '\t' | '\n' | '\x0C' | ' ' => {
                        // Ignore the character.
                    },
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the data state. Emit the current DOCTYPE token.
                        go!(self: emit_doctype; to State::Data)
                    },
                    // ↪ Anything else
                    _ => {
                        // This is an unexpected-character-after-doctype-system-identifier parse error.
                        // Reconsume in the bogus DOCTYPE state.
                        self.bad_char_error();
                        go!(self: reconsume BogusDoctype)
                    },
                }
            },

            // https://html.spec.whatwg.org/#bogus-doctype-state
            states::BogusDoctype => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the data state.
                        // Emit the current DOCTYPE token.
                        go!(self: emit_doctype; to State::Data)
                    },
                    // ↪ U+0000 NULL
                    '\0' => {
                        // This is an unexpected-null-character parse error.
                        // Ignore the character.
                        self.bad_char_error();
                    },
                    // ↪ Anything else
                    _ => {
                        // Ignore the character.
                    },
                }
            },

            // https://html.spec.whatwg.org/#cdata-section-state
            states::CdataSection => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+005D RIGHT SQUARE BRACKET (])
                    ']' => {
                        // Switch to the CDATA section bracket state.
                        go!(self: to State::CdataSectionBracket)
                    },
                    // FIXME: This is not in the specification.
                    '\0' => {
                        self.emit_temp_buf();
                        self.emit_char('\0');
                    },
                    // ↪ Anything else
                    character => {
                        // Emit the current input character as a character token.
                        go!(self: push_temp character)
                    },
                }
            },

            // https://html.spec.whatwg.org/#cdata-section-bracket-state
            states::CdataSectionBracket => {
                // Consume the next input character:
                match get_char!(self, input) {
                    // ↪ U+005D RIGHT SQUARE BRACKET (])
                    ']' => {
                        // Switch to the CDATA section end state.
                        go!(self: to State::CdataSectionEnd)
                    },
                    // ↪ Anything else
                    _ => {
                        // Emit a U+005D RIGHT SQUARE BRACKET character token. Reconsume in the CDATA section state.
                        go!(self: push_temp ']'; reconsume CdataSection)
                    },
                }
            },

            // https://html.spec.whatwg.org/#cdata-section-end-state
            states::CdataSectionEnd => loop {
                // Consume the next input character:
                match get_char!(self, input) {
                    // U+005D RIGHT SQUARE BRACKET (])
                    ']' => {
                        // Emit a U+005D RIGHT SQUARE BRACKET character token.
                        go!(self: push_temp ']')
                    },
                    // U+003E GREATER-THAN SIGN (>)
                    '>' => {
                        // Switch to the data state.
                        self.emit_temp_buf();
                        go!(self: to State::Data);
                    },
                    // Anything else
                    _ => {
                        // Emit two U+005D RIGHT SQUARE BRACKET character tokens.
                        // Reconsume in the CDATA section state.
                        go!(self: push_temp ']'; push_temp ']'; reconsume CdataSection)
                    },
                }
            },
            // TODO: What about the processing-instruction related states?
            //§ END
        }
    }

    fn step_char_ref_tokenizer(&self, input: &BufferQueue) -> ProcessResult<Sink::Handle> {
        let mut char_ref_tokenizer = self.char_ref_tokenizer.borrow_mut();
        let progress = match char_ref_tokenizer.as_mut().unwrap().step(self, input) {
            char_ref::Status::Done(char_ref) => {
                self.process_char_ref(char_ref);
                *char_ref_tokenizer = None;
                return ProcessResult::Continue;
            },

            char_ref::Status::Stuck => ProcessResult::Suspend,
            char_ref::Status::Progress => ProcessResult::Continue,
        };

        progress
    }

    fn process_char_ref(&self, char_ref: CharRef) {
        let CharRef {
            mut chars,
            mut num_chars,
        } = char_ref;

        if num_chars == 0 {
            chars[0] = '&';
            num_chars = 1;
        }

        for i in 0..num_chars {
            let c = chars[i as usize];
            match self.state.get() {
                states::Data | states::RawData(states::Rcdata) => self.emit_char(c),

                states::AttributeValue(_) => go!(self: push_value c),

                _ => panic!(
                    "state {:?} should not be reachable in process_char_ref",
                    self.state.get()
                ),
            }
        }
    }

    /// Indicate that we have reached the end of the input.
    pub fn end(&self) {
        // Handle EOF in the char ref sub-tokenizer, if there is one.
        // Do this first because it might un-consume stuff.
        let input = BufferQueue::default();
        match self.char_ref_tokenizer.take() {
            None => (),
            Some(mut tokenizer) => {
                self.process_char_ref(tokenizer.end_of_file(self, &input));
            },
        }

        // Process all remaining buffered input.
        // If we're waiting for lookahead, we're not gonna get it.
        self.at_eof.set(true);
        assert!(matches!(self.run(&input), TokenizerResult::Done));
        assert!(input.is_empty());

        loop {
            match self.eof_step() {
                ProcessResult::Continue => (),
                ProcessResult::Suspend => break,
                ProcessResult::Script(_) | ProcessResult::EncodingIndicator(_) => unreachable!(),
            }
        }

        self.sink.end();

        if self.opts.profile {
            self.dump_profile();
        }
    }

    fn dump_profile(&self) {
        let mut results: Vec<(states::State, u64)> = self
            .state_profile
            .borrow()
            .iter()
            .map(|(s, t)| (*s, *t))
            .collect();
        results.sort_by_key(|&(_, x)| Reverse(x));

        let total: u64 = results.iter().map(|&(_, t)| t).sum();
        println!("\nTokenizer profile, in nanoseconds");
        println!(
            "\n{:12}         total in token sink",
            self.time_in_sink.get()
        );
        println!("\n{total:12}         total in tokenizer");

        for (k, v) in results.into_iter() {
            let pct = 100.0 * (v as f64) / (total as f64);
            println!("{v:12}  {pct:4.1}%  {k:?}");
        }
    }

    fn eof_step(&self) -> ProcessResult<Sink::Handle> {
        debug!("processing EOF in state {:?}", self.state.get());
        match self.state.get() {
            states::Data
            | states::RawData(Rcdata)
            | states::RawData(Rawtext)
            | states::RawData(ScriptData)
            | states::Plaintext => go!(self: eof),

            states::TagName
            | states::RawData(ScriptDataEscaped(_))
            | states::BeforeAttributeName
            | states::AttributeName
            | states::AfterAttributeName
            | states::AttributeValue(_)
            | states::AfterAttributeValueQuoted
            | states::SelfClosingStartTag
            | states::ScriptDataEscapedDash(_)
            | states::ScriptDataEscapedDashDash(_) => {
                self.bad_eof_error();
                go!(self: to State::Data)
            },

            states::BeforeAttributeValue => go!(self: reconsume AttributeValue(Unquoted)),

            states::TagOpen => {
                self.bad_eof_error();
                self.emit_char('<');
                go!(self: to State::Data);
            },

            states::EndTagOpen => {
                self.bad_eof_error();
                self.emit_char('<');
                self.emit_char('/');
                go!(self: to State::Data);
            },

            states::RawLessThanSign(ScriptDataEscaped(DoubleEscaped)) => {
                go!(self: to State::RawData(ScriptDataEscaped(DoubleEscaped)))
            },

            states::RawLessThanSign(kind) => {
                self.emit_char('<');
                go!(self: to State::RawData(kind));
            },

            states::RawEndTagOpen(kind) => {
                self.emit_char('<');
                self.emit_char('/');
                go!(self: to State::RawData(kind));
            },

            states::RawEndTagName(kind) => {
                self.emit_char('<');
                self.emit_char('/');
                self.emit_temp_buf();
                go!(self: to State::RawData(kind))
            },

            states::ScriptDataEscapeStart(kind) => {
                go!(self: to State::RawData(ScriptDataEscaped(kind)))
            },

            states::ScriptDataEscapeStartDash => go!(self: to State::RawData(ScriptData)),

            states::ScriptDataDoubleEscapeEnd => {
                go!(self: to State::RawData(ScriptDataEscaped(DoubleEscaped)))
            },

            states::CommentStart
            | states::CommentStartDash
            | states::Comment
            | states::CommentEndDash
            | states::CommentEnd
            | states::CommentEndBang => {
                self.bad_eof_error();
                go!(self: emit_comment; to State::Data)
            },

            states::CommentLessThanSign | states::CommentLessThanSignBang => {
                go!(self: reconsume Comment)
            },

            states::CommentLessThanSignBangDash => go!(self: reconsume CommentEndDash),

            states::CommentLessThanSignBangDashDash => go!(self: reconsume CommentEnd),

            states::Doctype | states::BeforeDoctypeName => {
                self.bad_eof_error();
                go!(self: create_doctype; force_quirks; emit_doctype; to State::Data)
            },

            states::DoctypeName
            | states::AfterDoctypeName
            | states::AfterDoctypeKeyword(_)
            | states::BeforeDoctypeIdentifier(_)
            | states::DoctypeIdentifierDoubleQuoted(_)
            | states::DoctypeIdentifierSingleQuoted(_)
            | states::AfterDoctypeIdentifier(_)
            | states::BetweenDoctypePublicAndSystemIdentifiers => {
                self.bad_eof_error();
                go!(self: force_quirks; emit_doctype; to State::Data)
            },

            states::BogusDoctype => go!(self: emit_doctype; to State::Data),

            states::BogusComment => go!(self: emit_comment; to State::Data),

            states::MarkupDeclarationOpen => {
                self.bad_char_error();
                go!(self: to State::BogusComment)
            },

            states::CdataSection => {
                self.emit_temp_buf();
                self.bad_eof_error();
                go!(self: to State::Data)
            },

            states::CdataSectionBracket => go!(self: push_temp ']'; to State::CdataSection),

            states::CdataSectionEnd => {
                go!(self: push_temp ']'; push_temp ']'; to State::CdataSection)
            },
        }
    }

    /// Checks for supported SIMD feature, which is now either SSE2 for x86/x86_64 or NEON for aarch64.
    fn is_supported_simd_feature_detected() -> bool {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            is_x86_feature_detected!("sse2")
        }

        #[cfg(target_arch = "aarch64")]
        {
            std::arch::is_aarch64_feature_detected!("neon")
        }

        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
        false
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64"))]
    /// Implements the [data state] with SIMD instructions.
    /// Calls SSE2- or NEON-specific function for chunks and processes any remaining bytes.
    ///
    /// The algorithm implemented is the naive SIMD approach described [here].
    ///
    /// ### SAFETY:
    /// Calling this function on a CPU that supports neither SSE2 nor NEON causes undefined behaviour.
    ///
    /// [data state]: https://html.spec.whatwg.org/#data-state
    /// [here]: https://lemire.me/blog/2024/06/08/scan-html-faster-with-simd-instructions-chrome-edition/
    unsafe fn data_state_simd_fast_path(&self, input: &mut StrTendril) -> Option<SetResult> {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        let (mut i, mut n_newlines) = self.data_state_sse2_fast_path(input);

        #[cfg(target_arch = "aarch64")]
        let (mut i, mut n_newlines) = self.data_state_neon_fast_path(input);

        // Process any remaining bytes (less than STRIDE)
        while let Some(c) = input.as_bytes().get(i) {
            if matches!(*c, b'<' | b'&' | b'\r' | b'\0') {
                break;
            }
            if *c == b'\n' {
                n_newlines += 1;
            }

            i += 1;
        }

        let set_result = if i == 0 {
            let first_char = input.pop_front_char().unwrap();
            debug_assert!(matches!(first_char, '<' | '&' | '\r' | '\0'));

            // FIXME: Passing a bogus input queue is only relevant when c is \n, which can never happen in this case.
            // Still, it would be nice to not have to do that.
            // The same is true for the unwrap call.
            let preprocessed_char = self
                .get_preprocessed_char(first_char, &BufferQueue::default())
                .unwrap();
            SetResult::FromSet(preprocessed_char)
        } else {
            debug_assert!(
                input.len() >= i,
                "Trying to remove {:?} bytes from a tendril that is only {:?} bytes long",
                i,
                input.len()
            );
            let consumed_chunk = input.unsafe_subtendril(0, i as u32);
            input.unsafe_pop_front(i as u32);
            SetResult::NotFromSet(consumed_chunk)
        };

        self.current_line.set(self.current_line.get() + n_newlines);

        Some(set_result)
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "sse2")]
    /// Implements the [data state] with SSE2 instructions for x86/x86_64.
    /// Returns a pair of the number of bytes processed and the number of newlines found.
    ///
    /// ### SAFETY:
    /// Calling this function on a CPU that does not support NEON causes undefined behaviour.
    ///
    /// [data state]: https://html.spec.whatwg.org/#data-state
    unsafe fn data_state_sse2_fast_path(&self, input: &mut StrTendril) -> (usize, u64) {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::{
            __m128i, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_or_si128,
            _mm_set1_epi8,
        };
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::{
            __m128i, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_or_si128,
            _mm_set1_epi8,
        };

        debug_assert!(!input.is_empty());

        let quote_mask = _mm_set1_epi8('<' as i8);
        let escape_mask = _mm_set1_epi8('&' as i8);
        let carriage_return_mask = _mm_set1_epi8('\r' as i8);
        let zero_mask = _mm_set1_epi8('\0' as i8);
        let newline_mask = _mm_set1_epi8('\n' as i8);

        let raw_bytes: &[u8] = input.as_bytes();
        let start = raw_bytes.as_ptr();

        const STRIDE: usize = 16;
        let mut i = 0;
        let mut n_newlines = 0;
        while i + STRIDE <= raw_bytes.len() {
            // Load a 16 byte chunk from the input
            let data = _mm_loadu_si128(start.add(i) as *const __m128i);

            // Compare the chunk against each mask
            let quotes = _mm_cmpeq_epi8(data, quote_mask);
            let escapes = _mm_cmpeq_epi8(data, escape_mask);
            let carriage_returns = _mm_cmpeq_epi8(data, carriage_return_mask);
            let zeros = _mm_cmpeq_epi8(data, zero_mask);
            let newlines = _mm_cmpeq_epi8(data, newline_mask);

            // Combine all test results and create a bitmask from them.
            // Each bit in the mask will be 1 if the character at the bit position is in the set and 0 otherwise.
            let test_result = _mm_or_si128(
                _mm_or_si128(quotes, zeros),
                _mm_or_si128(escapes, carriage_returns),
            );
            let bitmask = _mm_movemask_epi8(test_result);
            let newline_mask = _mm_movemask_epi8(newlines);

            if (bitmask != 0) {
                // We have reached one of the characters that cause the state machine to transition
                let position = if cfg!(target_endian = "little") {
                    bitmask.trailing_zeros() as usize
                } else {
                    bitmask.leading_zeros() as usize
                };

                n_newlines += (newline_mask & ((1 << position) - 1)).count_ones() as u64;
                i += position;
                break;
            } else {
                n_newlines += newline_mask.count_ones() as u64;
            }

            i += STRIDE;
        }

        (i, n_newlines)
    }

    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    /// Implements the [data state] with NEON SIMD instructions for AArch64.
    /// Returns a pair of the number of bytes processed and the number of newlines found.
    ///
    /// ### SAFETY:
    /// Calling this function on a CPU that does not support NEON causes undefined behaviour.
    ///
    /// [data state]: https://html.spec.whatwg.org/#data-state
    unsafe fn data_state_neon_fast_path(&self, input: &mut StrTendril) -> (usize, u64) {
        use std::arch::aarch64::{vceqq_u8, vdupq_n_u8, vld1q_u8, vmaxvq_u8, vorrq_u8};

        debug_assert!(!input.is_empty());

        let quote_mask = vdupq_n_u8(b'<');
        let escape_mask = vdupq_n_u8(b'&');
        let carriage_return_mask = vdupq_n_u8(b'\r');
        let zero_mask = vdupq_n_u8(b'\0');
        let newline_mask = vdupq_n_u8(b'\n');

        let raw_bytes: &[u8] = input.as_bytes();
        let start = raw_bytes.as_ptr();

        const STRIDE: usize = 16;
        let mut i = 0;
        let mut n_newlines = 0;
        while i + STRIDE <= raw_bytes.len() {
            // Load a 16 byte chunk from the input
            let data = vld1q_u8(start.add(i));

            // Compare the chunk against each mask
            let quotes = vceqq_u8(data, quote_mask);
            let escapes = vceqq_u8(data, escape_mask);
            let carriage_returns = vceqq_u8(data, carriage_return_mask);
            let zeros = vceqq_u8(data, zero_mask);
            let newlines = vceqq_u8(data, newline_mask);

            // Combine all test results and create a bitmask from them.
            // Each bit in the mask will be 1 if the character at the bit position is in the set and 0 otherwise.
            let test_result =
                vorrq_u8(vorrq_u8(quotes, zeros), vorrq_u8(escapes, carriage_returns));
            let bitmask = vmaxvq_u8(test_result);
            let newline_mask = vmaxvq_u8(newlines);
            if bitmask != 0 {
                // We have reached one of the characters that cause the state machine to transition
                let chunk_bytes = std::slice::from_raw_parts(start.add(i), STRIDE);
                let position = chunk_bytes
                    .iter()
                    .position(|&b| matches!(b, b'<' | b'&' | b'\r' | b'\0'))
                    .unwrap();

                n_newlines += chunk_bytes[..position]
                    .iter()
                    .filter(|&&b| b == b'\n')
                    .count() as u64;

                i += position;
                break;
            } else if newline_mask != 0 {
                let chunk_bytes = std::slice::from_raw_parts(start.add(i), STRIDE);
                n_newlines += chunk_bytes.iter().filter(|&&b| b == b'\n').count() as u64;
            }

            i += STRIDE;
        }

        (i, n_newlines)
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use super::option_push; // private items
    use crate::tendril::{SliceExt, StrTendril};

    use super::{TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts};

    use super::interface::{CharacterTokens, EOFToken, NullCharacterToken, ParseError};
    use super::interface::{EndTag, StartTag, Tag, TagKind};
    use super::interface::{TagToken, Token};

    use markup5ever::buffer_queue::BufferQueue;
    use std::cell::RefCell;

    use crate::LocalName;

    // LinesMatch implements the TokenSink trait. It is used for testing to see
    // if current_line is being updated when process_token is called. The lines
    // vector is a collection of the line numbers that each token is on.
    struct LinesMatch {
        tokens: RefCell<Vec<Token>>,
        current_str: RefCell<StrTendril>,
        lines: RefCell<Vec<(Token, u64)>>,
    }

    impl LinesMatch {
        fn new() -> LinesMatch {
            LinesMatch {
                tokens: RefCell::new(vec![]),
                current_str: RefCell::new(StrTendril::new()),
                lines: RefCell::new(vec![]),
            }
        }

        fn push(&self, token: Token, line_number: u64) {
            self.finish_str();
            self.lines.borrow_mut().push((token, line_number));
        }

        fn finish_str(&self) {
            if !self.current_str.borrow().is_empty() {
                let s = self.current_str.take();
                self.tokens.borrow_mut().push(CharacterTokens(s));
            }
        }
    }

    impl TokenSink for LinesMatch {
        type Handle = ();

        fn process_token(&self, token: Token, line_number: u64) -> TokenSinkResult<Self::Handle> {
            match token {
                CharacterTokens(b) => {
                    self.current_str.borrow_mut().push_slice(&b);
                },

                NullCharacterToken => {
                    self.current_str.borrow_mut().push_char('\0');
                },

                ParseError(_) => {
                    panic!("unexpected parse error");
                },

                TagToken(mut t) => {
                    // The spec seems to indicate that one can emit
                    // erroneous end tags with attrs, but the test
                    // cases don't contain them.
                    match t.kind {
                        EndTag => {
                            t.self_closing = false;
                            t.attrs = vec![];
                        },
                        _ => t.attrs.sort_by(|a1, a2| a1.name.cmp(&a2.name)),
                    }
                    self.push(TagToken(t), line_number);
                },

                EOFToken => (),

                _ => self.push(token, line_number),
            }
            TokenSinkResult::Continue
        }
    }

    // Take in tokens, process them, and return vector with line
    // numbers that each token is on
    fn tokenize(input: Vec<StrTendril>, opts: TokenizerOpts) -> Vec<(Token, u64)> {
        let sink = LinesMatch::new();
        let tok = Tokenizer::new(sink, opts);
        let buffer = BufferQueue::default();
        for chunk in input.into_iter() {
            buffer.push_back(chunk);
            let _ = tok.feed(&buffer);
        }
        tok.end();
        tok.sink.lines.take()
    }

    // Create a tag token
    fn create_tag(token: StrTendril, tagkind: TagKind) -> Token {
        let name = LocalName::from(&*token);

        TagToken(Tag {
            kind: tagkind,
            name,
            self_closing: false,
            attrs: vec![],
            had_duplicate_attributes: false,
        })
    }

    #[test]
    fn push_to_None_gives_singleton() {
        let mut s: Option<StrTendril> = None;
        option_push(&mut s, 'x');
        assert_eq!(s, Some("x".to_tendril()));
    }

    #[test]
    fn push_to_empty_appends() {
        let mut s: Option<StrTendril> = Some(StrTendril::new());
        option_push(&mut s, 'x');
        assert_eq!(s, Some("x".to_tendril()));
    }

    #[test]
    fn push_to_nonempty_appends() {
        let mut s: Option<StrTendril> = Some(StrTendril::from_slice("y"));
        option_push(&mut s, 'x');
        assert_eq!(s, Some("yx".to_tendril()));
    }

    #[test]
    fn check_lines() {
        let opts = TokenizerOpts {
            exact_errors: false,
            discard_bom: true,
            profile: false,
            initial_state: None,
            last_start_tag_name: None,
        };
        let vector = vec![
            StrTendril::from("<a>\n"),
            StrTendril::from("<b>\n"),
            StrTendril::from("</b>\n"),
            StrTendril::from("</a>\n"),
        ];
        let expected = vec![
            (create_tag(StrTendril::from("a"), StartTag), 1),
            (create_tag(StrTendril::from("b"), StartTag), 2),
            (create_tag(StrTendril::from("b"), EndTag), 3),
            (create_tag(StrTendril::from("a"), EndTag), 4),
        ];
        let results = tokenize(vector, opts);
        assert_eq!(results, expected);
    }

    #[test]
    fn check_lines_with_new_line() {
        let opts = TokenizerOpts {
            exact_errors: false,
            discard_bom: true,
            profile: false,
            initial_state: None,
            last_start_tag_name: None,
        };
        let vector = vec![
            StrTendril::from("<a>\r\n"),
            StrTendril::from("<b>\r\n"),
            StrTendril::from("</b>\r\n"),
            StrTendril::from("</a>\r\n"),
        ];
        let expected = vec![
            (create_tag(StrTendril::from("a"), StartTag), 1),
            (create_tag(StrTendril::from("b"), StartTag), 2),
            (create_tag(StrTendril::from("b"), EndTag), 3),
            (create_tag(StrTendril::from("a"), EndTag), 4),
        ];
        let results = tokenize(vector, opts);
        assert_eq!(results, expected);
    }
}
