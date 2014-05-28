// Copyright 2014 The HTML5 for Rust Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub use self::interface::{Doctype, Attribute, AttrName, TagKind, StartTag, EndTag, Tag};
pub use self::interface::{Token, DoctypeToken, TagToken, CommentToken};
pub use self::interface::{CharacterTokens, NullCharacterToken, EOFToken, ParseError};
pub use self::interface::TokenSink;

use self::states::{RawLessThanSign, RawEndTagOpen, RawEndTagName};
use self::states::{Rcdata, Rawtext, ScriptData, ScriptDataEscaped};
use self::states::{Escaped, DoubleEscaped};
use self::states::{Unquoted, SingleQuoted, DoubleQuoted};
use self::states::{DoctypeIdKind, Public, System};

use self::char_ref::{CharRef, CharRefTokenizer};

use self::buffer_queue::{BufferQueue, DataRunOrChar, DataRun, OneChar};

use util::str::{lower_ascii, lower_ascii_letter, empty_str};
use util::atom::Atom;

use std::ascii::StrAsciiExt;
use std::mem::replace;
use std::iter::AdditiveIterator;
use std::default::Default;

use time::precise_time_ns;

use collections::hashmap::HashMap;

pub mod states;
mod interface;
mod char_ref;
mod buffer_queue;

fn option_push_char(opt_str: &mut Option<String>, c: char) {
    match *opt_str {
        Some(ref mut s) => s.push_char(c),
        None => *opt_str = Some(String::from_char(1, c)),
    }
}

/// Tokenizer options, with an impl for Default.
#[deriving(Clone)]
pub struct TokenizerOpts {
    /// Report all parse errors described in the spec, at some
    /// performance penalty?  Default: false
    pub exact_errors: bool,

    /// Discard a U+FEFF BYTE ORDER MARK if we see one at the beginning
    /// of the stream?  Default: true
    pub discard_bom: bool,

    /// Keep a record of how long we spent in each state?  Printed
    /// when end() is called.  Default: false
    pub profile: bool,

    /// Initial state override.  Only the test runner should use
    /// a non-None value!
    pub initial_state: Option<states::State>,

    /// Last start tag.  Only the test runner should use a
    /// non-None value!
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

pub struct Tokenizer<'sink, Sink> {
    /// Options controlling the behavior of the tokenizer.
    opts: TokenizerOpts,

    /// Destination for tokens we emit.
    sink: &'sink mut Sink,

    /// The abstract machine state as described in the spec.
    state: states::State,

    /// Input ready to be tokenized.
    input_buffers: BufferQueue,

    /// If Some(n), the abstract machine needs n available
    /// characters to continue.
    wait_for: Option<uint>,

    /// Are we at the end of the file, once buffers have been processed
    /// completely? This affects whether we will wait for lookahead or not.
    at_eof: bool,

    /// Tokenizer for character references, if we're tokenizing
    /// one at the moment.
    char_ref_tokenizer: Option<Box<CharRefTokenizer>>,

    /// Current input character.  Just consumed, may reconsume.
    current_char: char,

    /// Should we reconsume the current input character?
    reconsume: bool,

    /// Did we just consume \r, translating it to \n?  In that case we need
    /// to ignore the next character if it's \n.
    ignore_lf: bool,

    /// Discard a U+FEFF BYTE ORDER MARK if we see one?  Only done at the
    /// beginning of the stream.
    discard_bom: bool,

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
    current_attr_value: String,

    /// Current comment.
    current_comment: String,

    /// Current doctype token.
    current_doctype: Doctype,

    /// Last start tag name, for use in checking "appropriate end tag".
    last_start_tag_name: Option<String>,

    /// The "temporary buffer" mentioned in the spec.
    temp_buf: String,

    /// Record of how many ns we spent in each state, if profiling is enabled.
    state_profile: HashMap<states::State, u64>,
}

impl<'sink, Sink: TokenSink> Tokenizer<'sink, Sink> {
    pub fn new(sink: &'sink mut Sink, mut opts: TokenizerOpts) -> Tokenizer<'sink, Sink> {
        let start_tag_name = opts.last_start_tag_name.take();
        let state = *opts.initial_state.as_ref().unwrap_or(&states::Data);
        let discard_bom = opts.discard_bom;
        Tokenizer {
            opts: opts,
            sink: sink,
            state: state,
            wait_for: None,
            char_ref_tokenizer: None,
            input_buffers: BufferQueue::new(),
            at_eof: false,
            current_char: '\0',
            reconsume: false,
            ignore_lf: false,
            discard_bom: discard_bom,
            current_tag_kind: StartTag,
            current_tag_name: empty_str(),
            current_tag_self_closing: false,
            current_tag_attrs: vec!(),
            current_attr_name: empty_str(),
            current_attr_value: empty_str(),
            current_comment: empty_str(),
            current_doctype: Doctype::new(),
            last_start_tag_name: start_tag_name,
            temp_buf: empty_str(),
            state_profile: HashMap::new(),
        }
    }

    pub fn feed(&mut self, input: String) {
        if input.len() == 0 {
            return;
        }

        let pos = if self.discard_bom && input.as_slice().char_at(0) == '\ufeff' {
            self.discard_bom = false;
            3  // length of BOM in UTF-8
        } else {
            0
        };

        self.input_buffers.push_back(input, pos);
        self.run();
    }

    // Get the next input character, which might be the character
    // 'c' that we already consumed from the buffers.
    fn get_preprocessed_char(&mut self, mut c: char) -> Option<char> {
        if self.ignore_lf {
            self.ignore_lf = false;
            if c == '\n' {
                c = unwrap_or_return!(self.input_buffers.next(), None);
            }
        }

        if c == '\r' {
            self.ignore_lf = true;
            c = '\n';
        }

        if self.opts.exact_errors && match c as u32 {
            0x01..0x08 | 0x0B | 0x0E..0x1F | 0x7F..0x9F | 0xFDD0..0xFDEF => true,
            n if (n & 0xFFFE) == 0xFFFE => true,
            _ => false,
        } {
            let msg = format!("Bad character {:?}", c);
            self.emit_error(msg);
        }

        debug!("got character {:?}", c);
        self.current_char = c;
        Some(c)
    }

    // Get the next input character, if one is available.
    fn get_char(&mut self) -> Option<char> {
        if self.reconsume {
            self.reconsume = false;
            Some(self.current_char)
        } else {
            self.input_buffers.next()
                .and_then(|c| self.get_preprocessed_char(c))
        }
    }

    // In a data state, get a run of characters to process as data, or a single
    // character.
    fn get_data(&mut self) -> Option<DataRunOrChar> {
        if self.opts.exact_errors || self.reconsume || self.ignore_lf {
            return self.get_char().map(|x| OneChar(x));
        }

        let d = self.input_buffers.pop_data();
        debug!("got data {:?}", d);
        match d {
            Some(OneChar(c)) => self.get_preprocessed_char(c).map(|x| OneChar(x)),

            // NB: We don't set self.current_char for a DataRun.  It shouldn't matter
            // for the codepaths that use this.
            _ => d
        }
    }

    // If fewer than n characters are available, return None.
    // Otherwise check if they satisfy a predicate, and consume iff so.
    //
    // FIXME: we shouldn't need to consume and then put back
    //
    // FIXME: do input stream preprocessing.  It's probably okay not to,
    // because none of the strings we look ahead for contain characters
    // affected by it, but think about this more.
    fn lookahead_and_consume(&mut self, n: uint, p: |&str| -> bool) -> Option<bool> {
        match self.input_buffers.pop_front(n) {
            None if self.at_eof => {
                debug!("lookahead: requested {:u} characters not available and never will be", n);
                Some(false)
            }
            None => {
                debug!("lookahead: requested {:u} characters not available", n);
                self.wait_for = Some(n);
                None
            }
            Some(s) => {
                if p(s.as_slice()) {
                    debug!("lookahead: condition satisfied by {:?}", s);
                    // FIXME: set current input character?
                    Some(true)
                } else {
                    debug!("lookahead: condition not satisfied by {:?}", s);
                    self.unconsume(s);
                    Some(false)
                }
            }
        }
    }

    // Run the state machine for as long as we can.
    fn run(&mut self) {
        if self.opts.profile {
            let mut run = true;
            while run {
                let state = self.state;
                let t0 = precise_time_ns();
                run = self.step();
                let dt = precise_time_ns() - t0;
                self.state_profile.insert_or_update_with(state, dt, |_, x| *x += dt);
            }
        } else {
            while self.step() {
            }
        }
    }

    fn bad_char_error(&mut self) {
        let msg = format!("Saw {:?} in state {:?}", self.current_char, self.state);
        self.emit_error(msg);
    }

    fn bad_eof_error(&mut self) {
        let msg = format!("Saw EOF in state {:?}", self.state);
        self.emit_error(msg);
    }

    fn emit_char(&mut self, c: char) {
        self.sink.process_token(match c {
            '\0' => NullCharacterToken,
            _ => CharacterTokens(String::from_char(1, c)),
        });
    }

    // The string must not contain '\0'!
    fn emit_chars(&mut self, b: String) {
        self.sink.process_token(CharacterTokens(b));
    }

    fn emit_current_tag(&mut self) {
        self.finish_attribute();

        match self.current_tag_kind {
            StartTag => {
                self.last_start_tag_name = Some(self.current_tag_name.clone());
            }
            EndTag => {
                if !self.current_tag_attrs.is_empty() {
                    self.emit_error("Attributes on an end tag".to_owned());
                }
                if self.current_tag_self_closing {
                    self.emit_error("Self-closing end tag".to_owned());
                }
            }
        }

        self.sink.process_token(TagToken(Tag {
            kind: self.current_tag_kind,
            name: Atom::take_from_buf(&mut self.current_tag_name),
            self_closing: self.current_tag_self_closing,
            attrs: replace(&mut self.current_tag_attrs, vec!()),
        }));

        if self.current_tag_kind == StartTag {
            match self.sink.query_state_change() {
                None => (),
                Some(s) => self.state = s,
            }
        }
    }

    fn emit_temp_buf(&mut self) {
        // FIXME: Make sure that clearing on emit is spec-compatible.
        let buf = replace(&mut self.temp_buf, empty_str());
        self.emit_chars(buf);
    }

    fn clear_temp_buf(&mut self) {
        // Do this without a new allocation.
        self.temp_buf.truncate(0);
    }

    fn emit_current_comment(&mut self) {
        self.sink.process_token(CommentToken(
            replace(&mut self.current_comment, empty_str())));
    }


    fn discard_tag(&mut self) {
        self.current_tag_name = String::new();
        self.current_tag_self_closing = false;
        self.current_tag_attrs = vec!();
    }

    fn create_tag(&mut self, kind: TagKind, c: char) {
        self.discard_tag();
        self.current_tag_name.push_char(c);
        self.current_tag_kind = kind;
    }

    fn have_appropriate_end_tag(&self) -> bool {
        match self.last_start_tag_name.as_ref() {
            Some(last) =>
                (self.current_tag_kind == EndTag)
                && (self.current_tag_name.as_slice() == last.as_slice()),
            None => false,
        }
    }

    fn create_attribute(&mut self, c: char) {
        self.finish_attribute();

        self.current_attr_name.push_char(c);
    }

    fn finish_attribute(&mut self) {
        if self.current_attr_name.len() == 0 {
            return;
        }

        // Check for a duplicate attribute.
        // FIXME: the spec says we should error as soon as the name is finished.
        // FIXME: linear time search, do we care?
        let dup = {
            let name = self.current_attr_name.as_slice();
            self.current_tag_attrs.iter().any(|a| a.name.as_slice() == name)
        };

        if dup {
            self.emit_error("Duplicate attribute".to_owned());
            self.current_attr_name.truncate(0);
            self.current_attr_value.truncate(0);
        } else {
            self.current_tag_attrs.push(Attribute {
                name: AttrName::new(Atom::take_from_buf(&mut self.current_attr_name)),
                value: replace(&mut self.current_attr_value, empty_str()),
            });
        }
    }

    fn emit_current_doctype(&mut self) {
        self.sink.process_token(DoctypeToken(
            replace(&mut self.current_doctype, Doctype::new())));
    }

    fn doctype_id<'a>(&'a mut self, kind: DoctypeIdKind) -> &'a mut Option<String> {
        match kind {
            Public => &mut self.current_doctype.public_id,
            System => &mut self.current_doctype.system_id,
        }
    }

    fn clear_doctype_id(&mut self, kind: DoctypeIdKind) {
        let id = self.doctype_id(kind);
        match *id {
            Some(ref mut s) => s.truncate(0),
            None => *id = Some(empty_str()),
        }
    }

    fn consume_char_ref(&mut self, addnl_allowed: Option<char>) {
        // NB: The char ref tokenizer assumes we have an additional allowed
        // character iff we're tokenizing in an attribute value.
        self.char_ref_tokenizer = Some(box CharRefTokenizer::new(addnl_allowed));
    }

    fn emit_eof(&mut self) {
        self.sink.process_token(EOFToken);
    }

    fn peek(&mut self) -> Option<char> {
        if self.reconsume {
            Some(self.current_char)
        } else {
            self.input_buffers.peek()
        }
    }

    fn discard_char(&mut self) {
        let c = self.get_char();
        assert!(c.is_some());
    }

    fn unconsume(&mut self, buf: String) {
        self.input_buffers.push_front(buf);
    }

    fn emit_error(&mut self, error: String) {
        self.sink.process_token(ParseError(error));
    }
}

// Shorthand for common state machine behaviors.
macro_rules! shorthand (
    ( emit $c:expr                    ) => ( self.emit_char($c);                                   );
    ( create_tag $kind:expr $c:expr   ) => ( self.create_tag($kind, $c);                           );
    ( push_tag $c:expr                ) => ( self.current_tag_name.push_char($c);                  );
    ( discard_tag                     ) => ( self.discard_tag();                                   );
    ( push_temp $c:expr               ) => ( self.temp_buf.push_char($c);                          );
    ( emit_temp                       ) => ( self.emit_temp_buf();                                 );
    ( clear_temp                      ) => ( self.clear_temp_buf();                                );
    ( create_attr $c:expr             ) => ( self.create_attribute($c);                            );
    ( push_name $c:expr               ) => ( self.current_attr_name.push_char($c);                 );
    ( push_value $c:expr              ) => ( self.current_attr_value.push_char($c);                );
    ( push_comment $c:expr            ) => ( self.current_comment.push_char($c);                   );
    ( append_comment $c:expr          ) => ( self.current_comment.push_str($c);                    );
    ( emit_comment                    ) => ( self.emit_current_comment();                          );
    ( clear_comment                   ) => ( self.current_comment.truncate(0);                     );
    ( create_doctype                  ) => ( self.current_doctype = Doctype::new();                );
    ( push_doctype_name $c:expr       ) => ( option_push_char(&mut self.current_doctype.name, $c); );
    ( push_doctype_id $k:expr $c:expr ) => ( option_push_char(self.doctype_id($k), $c);            );
    ( clear_doctype_id $k:expr        ) => ( self.clear_doctype_id($k);                            );
    ( force_quirks                    ) => ( self.current_doctype.force_quirks = true;             );
    ( emit_doctype                    ) => ( self.emit_current_doctype();                          );
    ( error                           ) => ( self.bad_char_error();                                );
    ( error_eof                       ) => ( self.bad_eof_error();                                 );
)

// Tracing of tokenizer actions.  This adds significant bloat and compile time,
// so it's behind a cfg flag.
#[cfg(trace_tokenizer)]
macro_rules! sh_trace ( ( $($cmds:tt)* ) => ({
    debug!("  {:s}", stringify!($($cmds)*));
    shorthand!($($cmds)*);
}))

#[cfg(not(trace_tokenizer))]
macro_rules! sh_trace ( ( $($cmds:tt)* ) => ( shorthand!($($cmds)*) ) )

// A little DSL for sequencing shorthand actions.
macro_rules! go (
    // A pattern like $($cmd:tt)* ; $($rest:tt)* causes parse ambiguity.
    // We have to tell the parser how much lookahead we need.

    ( $a:tt                   ; $($rest:tt)* ) => ({ sh_trace!($a);          go!($($rest)*); });
    ( $a:tt $b:tt             ; $($rest:tt)* ) => ({ sh_trace!($a $b);       go!($($rest)*); });
    ( $a:tt $b:tt $c:tt       ; $($rest:tt)* ) => ({ sh_trace!($a $b $c);    go!($($rest)*); });
    ( $a:tt $b:tt $c:tt $d:tt ; $($rest:tt)* ) => ({ sh_trace!($a $b $c $d); go!($($rest)*); });

    // These can only come at the end.

    ( to $s:ident                   ) => ({ self.state = states::$s; return true;           });
    ( to $s:ident $k1:expr          ) => ({ self.state = states::$s($k1); return true;      });
    ( to $s:ident $k1:expr $k2:expr ) => ({ self.state = states::$s($k1($k2)); return true; });

    ( reconsume $s:ident                   ) => ({ self.reconsume = true; go!(to $s);         });
    ( reconsume $s:ident $k1:expr          ) => ({ self.reconsume = true; go!(to $s $k1);     });
    ( reconsume $s:ident $k1:expr $k2:expr ) => ({ self.reconsume = true; go!(to $s $k1 $k2); });

    ( consume_char_ref             ) => ({ self.consume_char_ref(None); return true;         });
    ( consume_char_ref $addnl:expr ) => ({ self.consume_char_ref(Some($addnl)); return true; });

    // We have a default next state after emitting a tag, but the sink can override.
    ( emit_tag $s:ident ) => ({
        self.state = states::$s;
        self.emit_current_tag();
        return true;
    });

    ( eof ) => ({ self.emit_eof(); return false; });

    // If nothing else matched, it's a single command
    ( $($cmd:tt)+ ) => ( sh_trace!($($cmd)+); );

    // or nothing.
    () => (());
)

macro_rules! go_match ( ( $x:expr, $($pats:pat)|+ => $($cmds:tt)* ) => (
    match $x {
        $($pats)|+ => go!($($cmds)*),
        _ => (),
    }
))

// This is a macro because it can cause early return
// from the function where it is used.
macro_rules! get_char ( () => (
    unwrap_or_return!(self.get_char(), false)
))

macro_rules! get_data ( () => (
    unwrap_or_return!(self.get_data(), false)
))

// NB: if you use this after get_char!() then the first char is still
// consumed no matter what!
macro_rules! lookahead_and_consume ( ($n:expr, $pred:expr) => (
    match self.lookahead_and_consume($n, $pred) {
        // This counts as progress because we set the
        // wait_for variable.
        None => return true,
        Some(r) => r
    }
))

impl<'sink, Sink: TokenSink> Tokenizer<'sink, Sink> {
    // Run the state machine for a while.
    // Return true if we should be immediately re-invoked
    // (this just simplifies control flow vs. break / continue).
    fn step(&mut self) -> bool {
        if self.char_ref_tokenizer.is_some() {
            return self.step_char_ref_tokenizer();
        }

        match self.wait_for {
            Some(n) if !self.input_buffers.has(n) => {
                debug!("lookahead: requested {:u} characters still not available", n);
                return false;
            }
            Some(n) => {
                debug!("lookahead: requested {:u} characters become available", n);
                self.wait_for = None;
            }
            None => (),
        }

        debug!("processing in state {:?}", self.state);
        match self.state {
            states::Data => loop { match get_data!() {
                DataRun(b)    => self.emit_chars(b),
                OneChar('&')  => go!(consume_char_ref),
                OneChar('<')  => go!(to TagOpen),
                OneChar('\0') => go!(error; emit '\0'),
                OneChar(c)    => go!(emit c),
            }},

            // RCDATA, RAWTEXT, script, or script escaped
            states::RawData(kind) => loop { match (get_data!(), kind) {
                (DataRun(b), _) => self.emit_chars(b),
                (OneChar('&'), Rcdata) => go!(consume_char_ref),
                (OneChar('-'), ScriptDataEscaped(esc_kind)) => go!(emit '-'; to ScriptDataEscapedDash esc_kind),
                (OneChar('<'), ScriptDataEscaped(DoubleEscaped)) => go!(emit '<'; to RawLessThanSign kind),
                (OneChar('<'), _) => go!(to RawLessThanSign kind),
                (OneChar('\0'), _) => go!(error; emit '\ufffd'),
                (OneChar(c), _) => go!(emit c),
            }},

            states::Plaintext => loop { match get_data!() {
                DataRun(b)    => self.emit_chars(b),
                OneChar('\0') => go!(error; emit '\ufffd'),
                OneChar(c)    => go!(emit c),
            }},

            states::TagOpen => loop { match get_char!() {
                '!' => go!(to MarkupDeclarationOpen),
                '/' => go!(to EndTagOpen),
                '?' => go!(error; clear_comment; push_comment '?'; to BogusComment),
                c => match lower_ascii_letter(c) {
                    Some(cl) => go!(create_tag StartTag cl; to TagName),
                    None     => go!(error; emit '<'; reconsume Data),
                }
            }},

            states::EndTagOpen => loop { match get_char!() {
                '>'  => go!(error; to Data),
                '\0' => go!(error; clear_comment; push_comment '\ufffd'; to BogusComment),
                c => match lower_ascii_letter(c) {
                    Some(cl) => go!(create_tag EndTag cl; to TagName),
                    None     => go!(error; clear_comment; push_comment c; to BogusComment),
                }
            }},

            states::TagName => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BeforeAttributeName),
                '/'  => go!(to SelfClosingStartTag),
                '>'  => go!(emit_tag Data),
                '\0' => go!(error; push_tag '\ufffd'),
                c    => go!(push_tag (lower_ascii(c))),
            }},

            states::RawLessThanSign(ScriptDataEscaped(Escaped)) => loop { match get_char!() {
                '/' => go!(clear_temp; to RawEndTagOpen ScriptDataEscaped Escaped),
                c => match lower_ascii_letter(c) {
                    Some(cl) => go!(clear_temp; push_temp cl; emit '<'; emit c;
                                    to ScriptDataEscapeStart DoubleEscaped),
                    None => go!(emit '<'; reconsume RawData ScriptDataEscaped Escaped),
                }
            }},

            states::RawLessThanSign(ScriptDataEscaped(DoubleEscaped)) => loop { match get_char!() {
                '/' => go!(clear_temp; to RawEndTagOpen ScriptDataEscaped DoubleEscaped),
                _   => go!(reconsume RawData ScriptDataEscaped DoubleEscaped),
            }},

            // otherwise
            states::RawLessThanSign(kind) => loop { match get_char!() {
                '/' => go!(clear_temp; to RawEndTagOpen kind),
                '!' if kind == ScriptData => go!(emit '<'; emit '!'; to ScriptDataEscapeStart Escaped),
                _   => go!(emit '<'; reconsume RawData Rcdata),
            }},

            states::RawEndTagOpen(kind) => loop {
                let c = get_char!();
                match lower_ascii_letter(c) {
                    Some(cl) => go!(create_tag EndTag cl; push_temp c; to RawEndTagName kind),
                    None     => go!(emit '<'; emit '/'; reconsume RawData kind),
                }
            },

            states::RawEndTagName(kind) => loop {
                let c = get_char!();
                if self.have_appropriate_end_tag() {
                    match c {
                        '\t' | '\n' | '\x0C' | ' '
                            => go!(to BeforeAttributeName),
                        '/' => go!(to SelfClosingStartTag),
                        '>' => go!(emit_tag Data),
                        _ => (),
                    }
                }

                match lower_ascii_letter(c) {
                    Some(cl) => go!(push_tag cl; push_temp c),
                    None     => go!(discard_tag; emit '<'; emit '/'; emit_temp; reconsume RawData kind),
                }
            },

            states::ScriptDataEscapeStart(DoubleEscaped) => loop {
                let c = get_char!();
                match c {
                    '\t' | '\n' | '\x0C' | ' ' | '/' | '>' => {
                        let esc = if self.temp_buf.as_slice() == "script" { DoubleEscaped } else { Escaped };
                        go!(emit c; to RawData ScriptDataEscaped esc);
                    }
                    _ => match lower_ascii_letter(c) {
                        Some(cl) => go!(push_temp cl; emit c),
                        None     => go!(reconsume RawData ScriptDataEscaped Escaped),
                    }
                }
            },

            states::ScriptDataEscapeStart(Escaped) => loop { match get_char!() {
                '-' => go!(emit '-'; to ScriptDataEscapeStartDash),
                _   => go!(reconsume RawData ScriptData),
            }},

            states::ScriptDataEscapeStartDash => loop { match get_char!() {
                '-' => go!(emit '-'; to ScriptDataEscapedDashDash Escaped),
                _   => go!(reconsume RawData ScriptData),
            }},

            states::ScriptDataEscapedDash(kind) => loop { match get_char!() {
                '-'  => go!(emit '-'; to ScriptDataEscapedDashDash kind),
                '<'  => {
                    if kind == DoubleEscaped { go!(emit '<'); }
                    go!(to RawLessThanSign ScriptDataEscaped kind);
                }
                '\0' => go!(error; emit '\ufffd'; to RawData ScriptDataEscaped kind),
                c    => go!(emit c; to RawData ScriptDataEscaped kind),
            }},

            states::ScriptDataEscapedDashDash(kind) => loop { match get_char!() {
                '-'  => go!(emit '-'),
                '<'  => {
                    if kind == DoubleEscaped { go!(emit '<'); }
                    go!(to RawLessThanSign ScriptDataEscaped kind);
                }
                '>'  => go!(emit '>'; to RawData ScriptData),
                '\0' => go!(error; emit '\ufffd'; to RawData ScriptDataEscaped kind),
                c    => go!(emit c; to RawData ScriptDataEscaped kind),
            }},

            states::ScriptDataDoubleEscapeEnd => loop {
                let c = get_char!();
                match c {
                    '\t' | '\n' | '\x0C' | ' ' | '/' | '>' => {
                        let esc = if self.temp_buf.as_slice() == "script" { Escaped } else { DoubleEscaped };
                        go!(emit c; to RawData ScriptDataEscaped esc);
                    }
                    _ => match lower_ascii_letter(c) {
                        Some(cl) => go!(push_temp cl; emit c),
                        None     => go!(reconsume RawData ScriptDataEscaped DoubleEscaped),
                    }
                }
            },

            states::BeforeAttributeName => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '/'  => go!(to SelfClosingStartTag),
                '>'  => go!(emit_tag Data),
                '\0' => go!(error; create_attr '\ufffd'; to AttributeName),
                c    => match lower_ascii_letter(c) {
                    Some(cl) => go!(create_attr cl; to AttributeName),
                    None => {
                        go_match!(c,
                            '"' | '\'' | '<' | '=' => error);
                        go!(create_attr c; to AttributeName);
                    }
                }
            }},

            states::AttributeName => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to AfterAttributeName),
                '/'  => go!(to SelfClosingStartTag),
                '='  => go!(to BeforeAttributeValue),
                '>'  => go!(emit_tag Data),
                '\0' => go!(error; push_name '\ufffd'),
                c    => match lower_ascii_letter(c) {
                    Some(cl) => go!(push_name cl),
                    None => {
                        go_match!(c,
                            '"' | '\'' | '<' => error);
                        go!(push_name c);
                    }
                }
            }},

            states::AfterAttributeName => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '/'  => go!(to SelfClosingStartTag),
                '='  => go!(to BeforeAttributeValue),
                '>'  => go!(emit_tag Data),
                '\0' => go!(error; create_attr '\ufffd'; to AttributeName),
                c    => match lower_ascii_letter(c) {
                    Some(cl) => go!(create_attr cl; to AttributeName),
                    None => {
                        go_match!(c,
                            '"' | '\'' | '<' => error);
                        go!(create_attr c; to AttributeName);
                    }
                }
            }},

            states::BeforeAttributeValue => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '"'  => go!(to AttributeValue DoubleQuoted),
                '&'  => go!(reconsume AttributeValue Unquoted),
                '\'' => go!(to AttributeValue SingleQuoted),
                '\0' => go!(error; push_value '\ufffd'; to AttributeValue Unquoted),
                '>'  => go!(error; emit_tag Data),
                c => {
                    go_match!(c,
                        '<' | '=' | '`' => error);
                    go!(push_value c; to AttributeValue Unquoted);
                }
            }},

            states::AttributeValue(DoubleQuoted) => loop { match get_char!() {
                '"'  => go!(to AfterAttributeValueQuoted),
                '&'  => go!(consume_char_ref '"'),
                '\0' => go!(error; push_value '\ufffd'),
                c    => go!(push_value c),
            }},

            states::AttributeValue(SingleQuoted) => loop { match get_char!() {
                '\'' => go!(to AfterAttributeValueQuoted),
                '&'  => go!(consume_char_ref '\''),
                '\0' => go!(error; push_value '\ufffd'),
                c    => go!(push_value c),
            }},

            states::AttributeValue(Unquoted) => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BeforeAttributeName),
                '&'  => go!(consume_char_ref '>'),
                '>'  => go!(emit_tag Data),
                '\0' => go!(error; push_value '\ufffd'),
                c    => {
                    go_match!(c,
                        '"' | '\'' | '<' | '=' | '`' => error);
                    go!(push_value c);
                }
            }},

            states::AfterAttributeValueQuoted => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BeforeAttributeName),
                '/'  => go!(to SelfClosingStartTag),
                '>'  => go!(emit_tag Data),
                _    => go!(error; reconsume BeforeAttributeName),
            }},

            states::SelfClosingStartTag => loop { match get_char!() {
                '>' => {
                    self.current_tag_self_closing = true;
                    go!(emit_tag Data);
                }
                _ => go!(error; reconsume BeforeAttributeName),
            }},

            states::CommentStart => loop { match get_char!() {
                '-'  => go!(to CommentStartDash),
                '\0' => go!(error; push_comment '\ufffd'; to Comment),
                '>'  => go!(error; emit_comment; to Data),
                c    => go!(push_comment c; to Comment),
            }},

            states::CommentStartDash => loop { match get_char!() {
                '-'  => go!(to CommentEnd),
                '\0' => go!(error; append_comment "-\ufffd"; to Comment),
                '>'  => go!(error; emit_comment; to Data),
                c    => go!(push_comment '-'; push_comment c; to Comment),
            }},

            states::Comment => loop { match get_char!() {
                '-'  => go!(to CommentEndDash),
                '\0' => go!(error; push_comment '\ufffd'),
                c    => go!(push_comment c),
            }},

            states::CommentEndDash => loop { match get_char!() {
                '-'  => go!(to CommentEnd),
                '\0' => go!(error; append_comment "-\ufffd"; to Comment),
                c    => go!(push_comment '-'; push_comment c; to Comment),
            }},

            states::CommentEnd => loop { match get_char!() {
                '>'  => go!(emit_comment; to Data),
                '\0' => go!(error; append_comment "--\ufffd"; to Comment),
                '!'  => go!(error; to CommentEndBang),
                '-'  => go!(error; push_comment '-'),
                c    => go!(error; append_comment "--"; push_comment c; to Comment),
            }},

            states::CommentEndBang => loop { match get_char!() {
                '-'  => go!(append_comment "--!"; to CommentEndDash),
                '>'  => go!(emit_comment; to Data),
                '\0' => go!(error; append_comment "--!\ufffd"; to Comment),
                c    => go!(append_comment "--!"; push_comment c; to Comment),
            }},

            states::Doctype => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                    => go!(to BeforeDoctypeName),
                _   => go!(error; reconsume BeforeDoctypeName),
            }},

            states::BeforeDoctypeName => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '\0' => go!(error; create_doctype; push_doctype_name '\ufffd'; to DoctypeName),
                '>'  => go!(error; create_doctype; force_quirks; emit_doctype; to Data),
                c    => go!(create_doctype; push_doctype_name (lower_ascii(c)); to DoctypeName),
            }},

            states::DoctypeName => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to AfterDoctypeName),
                '>'  => go!(emit_doctype; to Data),
                '\0' => go!(error; push_doctype_name '\ufffd'),
                c    => go!(push_doctype_name (lower_ascii(c))),
            }},

            states::AfterDoctypeName => loop { match () {
                _ if lookahead_and_consume!(6, |s| s.eq_ignore_ascii_case("public"))
                    => go!(to AfterDoctypeKeyword Public),
                _ if lookahead_and_consume!(6, |s| s.eq_ignore_ascii_case("system"))
                    => go!(to AfterDoctypeKeyword System),
                _ => match get_char!() {
                    '\t' | '\n' | '\x0C' | ' ' => (),
                    '>' => go!(emit_doctype; to Data),
                    _   => go!(error; force_quirks; to BogusDoctype),
                },
            }},

            states::AfterDoctypeKeyword(kind) => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BeforeDoctypeIdentifier kind),
                '"'  => go!(error; clear_doctype_id kind; to DoctypeIdentifierDoubleQuoted kind),
                '\'' => go!(error; clear_doctype_id kind; to DoctypeIdentifierSingleQuoted kind),
                '>'  => go!(error; force_quirks; emit_doctype; to Data),
                _    => go!(error; force_quirks; to BogusDoctype),
            }},

            states::BeforeDoctypeIdentifier(kind) => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '"'  => go!(clear_doctype_id kind; to DoctypeIdentifierDoubleQuoted kind),
                '\'' => go!(clear_doctype_id kind; to DoctypeIdentifierSingleQuoted kind),
                '>'  => go!(error; force_quirks; emit_doctype; to Data),
                _    => go!(error; force_quirks; to BogusDoctype),
            }},

            states::DoctypeIdentifierDoubleQuoted(kind) => loop { match get_char!() {
                '"'  => go!(to AfterDoctypeIdentifier kind),
                '\0' => go!(error; push_doctype_id kind '\ufffd'),
                '>'  => go!(error; force_quirks; emit_doctype; to Data),
                c    => go!(push_doctype_id kind c),
            }},

            states::DoctypeIdentifierSingleQuoted(kind) => loop { match get_char!() {
                '\'' => go!(to AfterDoctypeIdentifier kind),
                '\0' => go!(error; push_doctype_id kind '\ufffd'),
                '>'  => go!(error; force_quirks; emit_doctype; to Data),
                c    => go!(push_doctype_id kind c),
            }},

            states::AfterDoctypeIdentifier(Public) => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BetweenDoctypePublicAndSystemIdentifiers),
                '>'  => go!(emit_doctype; to Data),
                '"'  => go!(error; clear_doctype_id System; to DoctypeIdentifierDoubleQuoted System),
                '\'' => go!(error; clear_doctype_id System; to DoctypeIdentifierSingleQuoted System),
                _    => go!(error; force_quirks; to BogusDoctype),
            }},

            states::AfterDoctypeIdentifier(System) => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '>' => go!(emit_doctype; to Data),
                _   => go!(error; to BogusDoctype),
            }},

            states::BetweenDoctypePublicAndSystemIdentifiers => loop { match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '>'  => go!(emit_doctype; to Data),
                '"'  => go!(clear_doctype_id System; to DoctypeIdentifierDoubleQuoted System),
                '\'' => go!(clear_doctype_id System; to DoctypeIdentifierSingleQuoted System),
                _    => go!(error; force_quirks; to BogusDoctype),
            }},

            states::BogusDoctype => loop { match get_char!() {
                '>'  => go!(emit_doctype; to Data),
                _    => (),
            }},

            states::BogusComment => loop { match get_char!() {
                '>'  => go!(emit_comment; to Data),
                '\0' => go!(push_comment '\ufffd'),
                c    => go!(push_comment c),
            }},

            states::MarkupDeclarationOpen => loop { match () {
                _ if lookahead_and_consume!(2, |s| s == "--")
                    => go!(clear_comment; to CommentStart),
                _ if lookahead_and_consume!(7, |s| s.eq_ignore_ascii_case("doctype"))
                    => go!(to Doctype),
                // FIXME: CDATA, requires "adjusted current node" from tree builder
                // FIXME: 'error' gives wrong message
                _ => go!(error; to BogusComment),
            }},

            states::CdataSection
                => fail!("FIXME: state {:?} not implemented", self.state),
        }
    }

    fn step_char_ref_tokenizer(&mut self) -> bool {
        // FIXME HACK: Take and replace the tokenizer so we don't
        // double-mut-borrow self.  This is why it's boxed.
        let mut tok = self.char_ref_tokenizer.take_unwrap();
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

    fn process_char_ref(&mut self, char_ref: CharRef) {
        let CharRef { mut chars, mut num_chars } = char_ref;

        if num_chars == 0 {
            chars[0] = '&';
            num_chars = 1;
        }

        for i in range(0, num_chars) {
            let c = chars[i as uint];
            match self.state {
                states::Data | states::RawData(states::Rcdata)
                    => go!(emit c),

                states::AttributeValue(_)
                    => go!(push_value c),

                _ => fail!("state {:?} should not be reachable in process_char_ref", self.state),
            }
        }
    }

    pub fn end(&mut self) {
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
        self.wait_for = None;
        self.at_eof = true;
        self.run();

        while self.eof_step() {
            // loop
        }

        if self.opts.profile {
            let mut results: Vec<(states::State, u64)>
                = self.state_profile.iter().map(|(s, t)| (*s, *t)).collect();
            results.sort_by(|&(_, x), &(_, y)| y.cmp(&x));

            let total = results.iter().map(|&(_, t)| t).sum();
            println!("\nTokenizer profile, in nanoseconds");
            println!("\n{:12u}         total", total);

            for (k, v) in results.move_iter() {
                let pct = 100.0 * (v as f64) / (total as f64);
                println!("{:12u}  {:4.1f}%  {:?}", v, pct, k);
            }
        }
    }

    fn eof_step(&mut self) -> bool {
        debug!("processing EOF in state {:?}", self.state);
        match self.state {
            states::Data | states::RawData(Rcdata) | states::RawData(Rawtext)
            | states::RawData(ScriptData) | states::Plaintext
                => go!(eof),

            states::TagName | states::RawData(ScriptDataEscaped(_))
            | states::BeforeAttributeName | states::AttributeName
            | states::AfterAttributeName | states::BeforeAttributeValue
            | states::AttributeValue(_) | states::AfterAttributeValueQuoted
            | states::SelfClosingStartTag | states::ScriptDataEscapedDash(_)
            | states::ScriptDataEscapedDashDash(_)
                => go!(error_eof; to Data),

            states::TagOpen
                => go!(error_eof; emit '<'; to Data),

            states::EndTagOpen
                => go!(error_eof; emit '<'; emit '/'; to Data),

            states::RawLessThanSign(kind)
                => go!(emit '<'; to RawData kind),

            states::RawEndTagOpen(kind)
                => go!(emit '<'; emit '/'; to RawData kind),

            states::RawEndTagName(kind)
                => go!(emit '<'; emit '/'; emit_temp; to RawData kind),

            states::ScriptDataEscapeStart(kind)
                => go!(to RawData ScriptDataEscaped kind),

            states::ScriptDataEscapeStartDash
                => go!(to RawData ScriptData),

            states::ScriptDataDoubleEscapeEnd
                => go!(to RawData ScriptDataEscaped DoubleEscaped),

            states::CommentStart | states::CommentStartDash
            | states::Comment | states::CommentEndDash
            | states::CommentEnd | states::CommentEndBang
                => go!(error_eof; emit_comment; to Data),

            states::Doctype | states::BeforeDoctypeName
                => go!(error_eof; create_doctype; force_quirks; emit_doctype; to Data),

            states::DoctypeName | states::AfterDoctypeName | states::AfterDoctypeKeyword(_)
            | states::BeforeDoctypeIdentifier(_) | states::DoctypeIdentifierDoubleQuoted(_)
            | states::DoctypeIdentifierSingleQuoted(_) | states::AfterDoctypeIdentifier(_)
            | states::BetweenDoctypePublicAndSystemIdentifiers
                => go!(error_eof; force_quirks; emit_doctype; to Data),

            states::BogusDoctype
                => go!(emit_doctype; to Data),

            states::BogusComment
                => go!(emit_comment; to Data),

            states::MarkupDeclarationOpen
                => go!(error; to BogusComment),

            states::CdataSection
                => fail!("FIXME: state {:?} not implemented in EOF", self.state),
        }
    }
}


#[test]
fn push_to_None_gives_singleton() {
    let mut s: Option<String> = None;
    option_push_char(&mut s, 'x');
    assert_eq!(s, Some("x".to_strbuf()));
}

#[test]
fn push_to_empty_appends() {
    let mut s: Option<String> = Some(String::new());
    option_push_char(&mut s, 'x');
    assert_eq!(s, Some("x".to_strbuf()));
}

#[test]
fn push_to_nonempty_appends() {
    let mut s: Option<String> = Some("y".to_strbuf());
    option_push_char(&mut s, 'x');
    assert_eq!(s, Some("yx".to_strbuf()));
}
