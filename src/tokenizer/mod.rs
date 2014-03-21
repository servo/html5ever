/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

pub use self::tokens::{Doctype, Attribute, TagKind, StartTag, EndTag, Tag, Token};
pub use self::tokens::{DoctypeToken, TagToken, CommentToken};
pub use self::tokens::{CharacterToken, EOFToken, ParseError};

use self::states::{RawLessThanSign, RawEndTagOpen, RawEndTagName};
use self::states::{Rcdata, Rawtext, ScriptData, ScriptDataEscaped};
use self::states::{Escaped, DoubleEscaped};
use self::states::{Unquoted, SingleQuoted, DoubleQuoted};
use self::states::{DoctypeIdKind, Public, System};

use self::char_ref::{CharRef, CharRefTokenizer};

use util::buffer_queue::BufferQueue;
use util::ascii::{lower_ascii, lower_ascii_letter};

use std::str;
use std::ascii::StrAsciiExt;
use std::mem::replace;

mod tokens;
mod states;
mod char_ref;

pub trait TokenSink {
    fn process_token(&mut self, token: Token);
}

fn option_push_char(opt_str: &mut Option<~str>, c: char) {
    match *opt_str {
        Some(ref mut s) => s.push_char(c),
        None => *opt_str = Some(str::from_char(c)),
    }
}

pub struct Tokenizer<'sink, Sink> {
    /// Destination for tokens we emit.
    priv sink: &'sink mut Sink,

    /// The abstract machine state as described in the spec.
    priv state: states::State,

    /// Input ready to be tokenized.
    priv input_buffers: BufferQueue,

    /// If Some(n), the abstract machine needs n available
    /// characters to continue.
    priv wait_for: Option<uint>,

    /// Tokenizer for character references, if we're tokenizing
    /// one at the moment.
    priv char_ref_tokenizer: Option<~CharRefTokenizer>,

    /// Current input character.  Just consumed, may reconsume.
    priv current_char: char,

    /// Should we reconsume the current input character?
    priv reconsume: bool,

    // FIXME: The state machine guarantees the tag exists when
    // we need it, so we could eliminate the Option overhead.
    // Leaving it as Option for now, to find bugs.
    /// Current tag.
    priv current_tag: Option<Tag>,

    /// Current attribute.
    priv current_attr: Attribute,

    /// Current comment.
    priv current_comment: ~str,

    /// Current doctype token.
    priv current_doctype: Doctype,

    /// Last start tag name, for use in checking "appropriate end tag".
    priv last_start_tag_name: Option<~str>,

    /// The "temporary buffer" mentioned in the spec.
    priv temp_buf: ~str,
}

impl<'sink, Sink: TokenSink> Tokenizer<'sink, Sink> {
    pub fn new(sink: &'sink mut Sink) -> Tokenizer<'sink, Sink> {
        Tokenizer {
            sink: sink,
            state: states::Data,
            wait_for: None,
            char_ref_tokenizer: None,
            input_buffers: BufferQueue::new(),
            current_char: '\0',
            reconsume: false,
            current_tag: None,
            current_attr: Attribute::new(),
            current_comment: ~"",
            current_doctype: Doctype::new(),
            last_start_tag_name: None,
            temp_buf: ~"",
        }
    }

    pub fn feed(&mut self, input: ~str) {
        self.input_buffers.push_back(input);
        self.run();
    }

    // Get the next input character, if one is available.
    fn get_char(&mut self) -> Option<char> {
        if self.reconsume {
            self.reconsume = false;
            return Some(self.current_char);
        }

        self.input_buffers.next().map(|c| {
            self.current_char = c;
            c
        })
    }

    // If fewer than n characters are available, return None.
    // Otherwise check if they satisfy a predicate, and consume iff so.
    // FIXME: we shouldn't need to consume and then put back
    fn lookahead_and_consume(&mut self, n: uint, p: |&str| -> bool) -> Option<bool> {
        match self.input_buffers.pop_front(n) {
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
                    self.input_buffers.push_front(s);
                    Some(false)
                }
            }
        }
    }

    // Run the state machine for as long as we can.
    fn run(&mut self) {
        while self.step() {
        }
    }

    fn emit_error(&mut self, error: ~str) {
        self.sink.process_token(ParseError(error));
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
        self.sink.process_token(CharacterToken(c));
    }

    fn emit_current_tag(&mut self) {
        self.finish_attribute();

        let tag = self.current_tag.take().unwrap();
        match tag.kind {
            StartTag => self.last_start_tag_name = Some(tag.name.clone()),
            _ => ()
        }
        self.sink.process_token(TagToken(tag));
    }

    fn emit_temp_buf(&mut self) {
        // FIXME: Add a multi-character token and move temp_buf into it.
        //
        // Need to make sure that clearing on emit is spec-compatible.
        let buf = replace(&mut self.temp_buf, ~"");
        for c in buf.chars() {
            self.emit_char(c);
        }
    }

    fn clear_temp_buf(&mut self) {
        // Do this without a new allocation.
        self.temp_buf.truncate(0);
    }

    fn emit_current_comment(&mut self) {
        self.sink.process_token(CommentToken(
            replace(&mut self.current_comment, ~"")));
    }

    fn create_tag(&mut self, kind: TagKind, c: char) {
        assert!(self.current_tag.is_none());
        let mut t = Tag::new(kind);
        t.name.push_char(c);
        self.current_tag = Some(t);
    }

    fn tag<'t>(&'t self) -> &'t Tag {
        // Only use this from places where the state machine guarantees we have a tag
        self.current_tag.get_ref()
    }

    fn tag_mut<'t>(&'t mut self) -> &'t mut Tag {
        self.current_tag.get_mut_ref()
    }

    fn have_appropriate_end_tag(&self) -> bool {
        match (self.last_start_tag_name.as_ref(), self.current_tag.as_ref()) {
            (Some(last), Some(tag)) =>
                (tag.kind == EndTag) && (tag.name.as_slice() == last.as_slice()),
            _ => false
        }
    }

    fn create_attribute(&mut self, c: char) {
        self.finish_attribute();

        let attr = &mut self.current_attr;
        attr.name.push_char(c);
    }

    fn finish_attribute(&mut self) {
        if self.current_attr.name.len() == 0 {
            return;
        }

        // Check for a duplicate attribute.
        // FIXME: the spec says we should error as soon as the name is finished.
        // FIXME: linear time search, do we care?
        let dup = {
            let name = self.current_attr.name.as_slice();
            self.tag().attrs.iter().any(|a| a.name.as_slice() == name)
        };

        if dup {
            self.emit_error(~"Duplicate attribute");
            self.current_attr.clear();
        } else {
            let attr = replace(&mut self.current_attr, Attribute::new());
            self.tag_mut().attrs.push(attr);
        }
    }

    fn emit_current_doctype(&mut self) {
        self.sink.process_token(DoctypeToken(
            replace(&mut self.current_doctype, Doctype::new())));
    }

    fn doctype_id<'a>(&'a mut self, kind: DoctypeIdKind) -> &'a mut Option<~str> {
        match kind {
            Public => &mut self.current_doctype.public_id,
            System => &mut self.current_doctype.system_id,
        }
    }

    fn clear_doctype_id(&mut self, kind: DoctypeIdKind) {
        let id = self.doctype_id(kind);
        match *id {
            Some(ref mut s) => s.truncate(0),
            None => *id = Some(~""),
        }
    }

    fn consume_char_ref(&mut self, addnl_allowed: Option<char>) {
        // NB: The char ref tokenizer assumes we have an additional allowed
        // character iff we're tokenizing in an attribute value.
        self.char_ref_tokenizer = Some(~CharRefTokenizer::new(addnl_allowed));
    }

    fn emit_eof(&mut self) {
        self.sink.process_token(EOFToken);
    }
}

// Shorthand for common state machine behaviors.
macro_rules! shorthand (
    ( to $s:ident                     ) => ( self.state = states::$s;                              );
    ( to $s:ident $k1:expr            ) => ( self.state = states::$s($k1);                         );
    ( to $s:ident $k1:expr $k2:expr   ) => ( self.state = states::$s($k1($k2));                    );
    ( emit $c:expr                    ) => ( self.emit_char($c);                                   );
    ( create_tag $kind:expr $c:expr   ) => ( self.create_tag($kind, $c);                           );
    ( push_tag $c:expr                ) => ( self.tag_mut().name.push_char($c);                    );
    ( emit_tag                        ) => ( self.emit_current_tag();                              );
    ( push_temp $c:expr               ) => ( self.temp_buf.push_char($c);                          );
    ( emit_temp                       ) => ( self.emit_temp_buf();                                 );
    ( clear_temp                      ) => ( self.clear_temp_buf();                                );
    ( create_attr $c:expr             ) => ( self.create_attribute($c);                            );
    ( push_name $c:expr               ) => ( self.current_attr.name.push_char($c);                 );
    ( push_value $c:expr              ) => ( self.current_attr.value.push_char($c);                );
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
    ( consume_char_ref                ) => ( self.consume_char_ref(None);                          );
    ( consume_char_ref $addnl:expr    ) => ( self.consume_char_ref(Some($addnl));                  );
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
    // FIXME: Come up with a better name for 'finish'.
    ( reconsume ) => ({ self.reconsume = true; return true; });
    ( finish    ) => ( return true; );
    ( eof       ) => ({ self.emit_eof(); return; });

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
    // Run one step of the state machine.  Return true if we made any progress.
    //
    // FIXME: explicitly represent the EOF character?
    // For now the plan is to handle EOF in a separate match.
    fn step(&mut self) -> bool {
        if self.char_ref_tokenizer.is_some() {
            return self.step_char_ref_tokenizer();
        }

        match self.wait_for {
            Some(n) if !self.input_buffers.has(n) => return false,
            _ => self.wait_for = None,
        }

        match self.state {
            states::Data => match get_char!() {
                '&'  => go!(consume_char_ref),
                '<'  => go!(to TagOpen),
                '\0' => go!(error; emit '\0'),
                c    => go!(emit c),
            },

            // RCDATA, RAWTEXT, script, or script escaped
            states::RawData(kind) => match (get_char!(), kind) {
                ('&', Rcdata) => go!(consume_char_ref),
                ('-', ScriptDataEscaped(esc_kind)) => go!(to ScriptDataEscapedDash esc_kind; emit '-'),
                ('<', ScriptDataEscaped(DoubleEscaped)) => go!(to RawLessThanSign kind; emit '<'),
                ('<',  _) => go!(to RawLessThanSign kind),
                ('\0', _) => go!(error; emit '\ufffd'),
                (c,    _) => go!(emit c),
            },

            states::Plaintext => match get_char!() {
                '\0' => go!(error; emit '\ufffd'),
                c    => go!(emit c),
            },

            states::TagOpen => match get_char!() {
                '!' => go!(to MarkupDeclarationOpen),
                '/' => go!(to EndTagOpen),
                '?' => go!(error; clear_comment; push_comment '?'; to BogusComment),
                c => match lower_ascii_letter(c) {
                    Some(cl) => go!(create_tag StartTag cl; to TagName),
                    None     => go!(error; emit '<'; to Data; reconsume),
                }
            },

            states::EndTagOpen => match get_char!() {
                '>'  => go!(error; to Data),
                '\0' => go!(error; clear_comment; push_comment '\ufffd'; to BogusComment),
                c => match lower_ascii_letter(c) {
                    Some(cl) => go!(create_tag EndTag cl; to TagName),
                    None     => go!(error; clear_comment; push_comment c; to BogusComment),
                }
            },

            states::TagName => match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BeforeAttributeName),
                '/'  => go!(to SelfClosingStartTag),
                '>'  => go!(emit_tag; to Data),
                '\0' => go!(error; push_tag '\ufffd'),
                c    => go!(push_tag (lower_ascii(c))),
            },

            states::RawLessThanSign(ScriptDataEscaped(Escaped)) => match get_char!() {
                '/' => go!(clear_temp; to RawEndTagOpen ScriptDataEscaped Escaped),
                c => match lower_ascii_letter(c) {
                    Some(cl) => go!(clear_temp; push_temp cl;
                                    to ScriptDataEscapeStart DoubleEscaped; emit '<'; emit c),
                    None => go!(to RawData ScriptDataEscaped Escaped; emit '<'; reconsume),
                }
            },

            states::RawLessThanSign(ScriptDataEscaped(DoubleEscaped)) => match get_char!() {
                '/' => go!(clear_temp; to RawEndTagOpen ScriptDataEscaped DoubleEscaped),
                _   => go!(to RawData ScriptDataEscaped DoubleEscaped; reconsume),
            },

            // otherwise
            states::RawLessThanSign(kind) => match get_char!() {
                '/' => go!(clear_temp; to RawEndTagOpen kind),
                '!' if kind == ScriptData => go!(to ScriptDataEscapeStart Escaped; emit '<'; emit '!'),
                _   => go!(to RawData Rcdata; emit '<'; reconsume),
            },

            states::RawEndTagOpen(kind) => {
                let c = get_char!();
                match lower_ascii_letter(c) {
                    Some(cl) => go!(create_tag EndTag cl; push_temp c; to RawEndTagName kind),
                    None     => go!(to RawData kind; emit '<'; emit '/'; reconsume),
                }
            },

            states::RawEndTagName(kind) => {
                let c = get_char!();
                if self.have_appropriate_end_tag() {
                    match c {
                        '\t' | '\n' | '\x0C' | ' '
                            => go!(to BeforeAttributeName; finish),
                        '/' => go!(to SelfClosingStartTag; finish),
                        '>' => go!(emit_tag; to Data; finish),
                        // All of the above end with a return from this function.

                        _ => (),
                    }
                }

                match lower_ascii_letter(c) {
                    Some(cl) => go!(push_tag cl; push_temp c),
                    None     => go!(emit '<'; emit '/'; emit_temp; to RawData kind; reconsume),
                }
            },

            states::ScriptDataEscapeStart(DoubleEscaped) => {
                let c = get_char!();
                match c {
                    '\t' | '\n' | '\x0C' | ' ' | '/' | '>' => {
                        let esc = if self.temp_buf.as_slice() == "script" { DoubleEscaped } else { Escaped };
                        go!(to RawData ScriptDataEscaped esc; emit c);
                    }
                    _ => match lower_ascii_letter(c) {
                        Some(cl) => go!(push_temp cl; emit c),
                        None     => go!(to RawData ScriptDataEscaped Escaped; reconsume),
                    }
                }
            },

            states::ScriptDataEscapeStart(Escaped) => match get_char!() {
                '-' => go!(to ScriptDataEscapeStartDash; emit '-'),
                _   => go!(to RawData ScriptData; reconsume),
            },

            states::ScriptDataEscapeStartDash => match get_char!() {
                '-' => go!(to ScriptDataEscapedDashDash Escaped; emit '-'),
                _   => go!(to RawData ScriptData; reconsume),
            },

            states::ScriptDataEscapedDash(kind) => match get_char!() {
                '-'  => go!(to ScriptDataEscapedDashDash kind; emit '-'),
                '<'  => {
                    go!(to RawLessThanSign ScriptDataEscaped kind);
                    if kind == DoubleEscaped { go!(emit '<'); }
                }
                '\0' => go!(error; to RawData ScriptDataEscaped kind; emit '\ufffd'),
                c    => go!(to RawData ScriptDataEscaped kind; emit c),
            },

            states::ScriptDataEscapedDashDash(kind) => match get_char!() {
                '-'  => go!(emit '-'),
                '<'  => {
                    go!(to RawLessThanSign ScriptDataEscaped kind);
                    if kind == DoubleEscaped { go!(emit '<'); }
                }
                '>'  => go!(to RawData ScriptData; emit '>'),
                '\0' => go!(error; to RawData ScriptDataEscaped kind; emit '\ufffd'),
                c    => go!(to RawData ScriptDataEscaped kind; emit c),
            },

            states::ScriptDataDoubleEscapeEnd => {
                let c = get_char!();
                match c {
                    '\t' | '\n' | '\x0C' | ' ' | '/' | '>' => {
                        let esc = if self.temp_buf.as_slice() == "script" { Escaped } else { DoubleEscaped };
                        go!(to RawData ScriptDataEscaped esc; emit c);
                    }
                    _ => match lower_ascii_letter(c) {
                        Some(cl) => go!(push_temp cl; emit c),
                        None     => go!(to RawData ScriptDataEscaped DoubleEscaped; reconsume),
                    }
                }
            },

            states::BeforeAttributeName => match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '/'  => go!(to SelfClosingStartTag),
                '>'  => go!(to Data; emit_tag),
                '\0' => go!(error; create_attr '\ufffd'; to AttributeName),
                c    => match lower_ascii_letter(c) {
                    Some(cl) => go!(create_attr cl; to AttributeName),
                    None => {
                        go_match!(c,
                            '"' | '\'' | '<' | '=' => error);
                        go!(create_attr c; to AttributeName);
                    }
                }
            },

            states::AttributeName => match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to AfterAttributeName),
                '/'  => go!(to SelfClosingStartTag),
                '='  => go!(to BeforeAttributeValue),
                '>'  => go!(to Data; emit_tag),
                '\0' => go!(error; push_name '\ufffd'),
                c    => match lower_ascii_letter(c) {
                    Some(cl) => go!(push_name cl),
                    None => {
                        go_match!(c,
                            '"' | '\'' | '<' => error);
                        go!(push_name c);
                    }
                }
            },

            states::AfterAttributeName => match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '/'  => go!(to SelfClosingStartTag),
                '='  => go!(to BeforeAttributeValue),
                '>'  => go!(to Data; emit_tag),
                '\0' => go!(error; create_attr '\ufffd'; to AttributeName),
                c    => match lower_ascii_letter(c) {
                    Some(cl) => go!(create_attr cl; to AttributeName),
                    None => {
                        go_match!(c,
                            '"' | '\'' | '<' => error);
                        go!(create_attr c; to AttributeName);
                    }
                }
            },

            states::BeforeAttributeValue => match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '"'  => go!(to AttributeValue DoubleQuoted),
                '&'  => go!(to AttributeValue Unquoted; reconsume),
                '\'' => go!(to AttributeValue SingleQuoted),
                '\0' => go!(error; push_value '\ufffd'; to AttributeValue Unquoted),
                '>'  => go!(error; to Data; emit_tag),
                c => {
                    go_match!(c,
                        '<' | '=' | '`' => error);
                    go!(push_value c; to AttributeValue Unquoted);
                }
            },

            states::AttributeValue(DoubleQuoted) => match get_char!() {
                '"'  => go!(to AfterAttributeValueQuoted),
                '&'  => go!(consume_char_ref '"'),
                '\0' => go!(error; push_value '\ufffd'),
                c    => go!(push_value c),
            },

            states::AttributeValue(SingleQuoted) => match get_char!() {
                '\'' => go!(to AfterAttributeValueQuoted),
                '&'  => go!(consume_char_ref '\''),
                '\0' => go!(error; push_value '\ufffd'),
                c    => go!(push_value c),
            },

            states::AttributeValue(Unquoted) => match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BeforeAttributeName),
                '&'  => go!(consume_char_ref '>'),
                '>'  => go!(to Data; emit_tag),
                '\0' => go!(error; push_value '\ufffd'),
                c    => {
                    go_match!(c,
                        '"' | '\'' | '<' | '=' | '`' => error);
                    go!(push_value c);
                }
            },

            states::AfterAttributeValueQuoted => match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BeforeAttributeName),
                '/'  => go!(to SelfClosingStartTag),
                '>'  => go!(to Data; emit_tag),
                _    => go!(error; to BeforeAttributeName; reconsume),
            },

            states::SelfClosingStartTag => match get_char!() {
                '>' => {
                    self.tag_mut().self_closing = true;
                    go!(to Data; emit_tag);
                }
                _ => go!(error; to BeforeAttributeName; reconsume),
            },

            states::CommentStart => match get_char!() {
                '-'  => go!(to CommentStartDash),
                '\0' => go!(error; push_comment '\ufffd'; to Comment),
                '>'  => go!(error; to Data; emit_comment),
                c    => go!(push_comment c; to Comment),
            },

            states::CommentStartDash => match get_char!() {
                '-'  => go!(to CommentEnd),
                '\0' => go!(error; append_comment "-\ufffd"; to Comment),
                '>'  => go!(error; to Data; emit_comment),
                c    => go!(push_comment '-'; push_comment c; to Comment),
            },

            states::Comment => match get_char!() {
                '-'  => go!(to CommentEndDash),
                '\0' => go!(error; push_comment '\ufffd'),
                c    => go!(push_comment c),
            },

            states::CommentEndDash => match get_char!() {
                '-'  => go!(to CommentEnd),
                '\0' => go!(error; append_comment "-\ufffd"; to Comment),
                c    => go!(push_comment '-'; push_comment c; to Comment),
            },

            states::CommentEnd => match get_char!() {
                '>'  => go!(to Data; emit_comment),
                '\0' => go!(append_comment "--\ufffd"; to Comment),
                '!'  => go!(error; to CommentEndBang),
                '-'  => go!(error; push_comment '-'),
                c    => go!(error; append_comment "--"; push_comment c; to Comment),
            },

            states::CommentEndBang => match get_char!() {
                '-'  => go!(append_comment "--!"; to CommentEndDash),
                '>'  => go!(to Data; emit_comment),
                '\0' => go!(error; append_comment "--!\ufffd"; to Comment),
                c    => go!(append_comment "--!"; push_comment c; to Comment),
            },

            states::Doctype => match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                    => go!(to BeforeDoctypeName),
                _   => go!(error; to BeforeDoctypeName; reconsume),
            },

            states::BeforeDoctypeName => match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '\0' => go!(error; create_doctype; push_doctype_name '\ufffd'; to DoctypeName),
                '>'  => go!(error; create_doctype; force_quirks; to Data; emit_doctype),
                c    => go!(create_doctype; push_doctype_name (lower_ascii(c)); to DoctypeName),
            },

            states::DoctypeName => match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to AfterDoctypeName),
                '>'  => go!(to Data; emit_doctype),
                '\0' => go!(error; push_doctype_name '\ufffd'),
                c    => go!(push_doctype_name (lower_ascii(c))),
            },

            states::AfterDoctypeName => match () {
                _ if lookahead_and_consume!(6, |s| s.eq_ignore_ascii_case("public"))
                    => go!(to AfterDoctypeKeyword Public),
                _ if lookahead_and_consume!(6, |s| s.eq_ignore_ascii_case("system"))
                    => go!(to AfterDoctypeKeyword System),
                _ => match get_char!() {
                    '\t' | '\n' | '\x0C' | ' ' => (),
                    '>' => go!(to Data; emit_doctype),
                    _   => go!(error; force_quirks; to BogusDoctype),
                },
            },

            states::AfterDoctypeKeyword(kind) => match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BeforeDoctypeIdentifier kind),
                '"'  => go!(error; clear_doctype_id kind; to DoctypeIdentifierDoubleQuoted kind),
                '\'' => go!(error; clear_doctype_id kind; to DoctypeIdentifierSingleQuoted kind),
                '>'  => go!(error; force_quirks; to Data; emit_doctype),
                _    => go!(error; force_quirks; to BogusDoctype),
            },

            states::BeforeDoctypeIdentifier(kind) => match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '"'  => go!(clear_doctype_id kind; to DoctypeIdentifierDoubleQuoted kind),
                '\'' => go!(clear_doctype_id kind; to DoctypeIdentifierSingleQuoted kind),
                '>'  => go!(error; force_quirks; to Data; emit_doctype),
                _    => go!(error; force_quirks; to BogusDoctype),
            },

            states::DoctypeIdentifierDoubleQuoted(kind) => match get_char!() {
                '"'  => go!(to AfterDoctypeIdentifier kind),
                '\0' => go!(error; push_doctype_id kind '\ufffd'),
                '>'  => go!(error; force_quirks; to Data; emit_doctype),
                c    => go!(push_doctype_id kind c),
            },

            states::DoctypeIdentifierSingleQuoted(kind) => match get_char!() {
                '\'' => go!(to AfterDoctypeIdentifier kind),
                '\0' => go!(error; push_doctype_id kind '\ufffd'),
                '>'  => go!(error; force_quirks; to Data; emit_doctype),
                c    => go!(push_doctype_id kind c),
            },

            states::AfterDoctypeIdentifier(Public) => match get_char!() {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BetweenDoctypePublicAndSystemIdentifiers),
                '>'  => go!(to Data; emit_doctype),
                '"'  => go!(error; clear_doctype_id System; to DoctypeIdentifierDoubleQuoted System),
                '\'' => go!(error; clear_doctype_id System; to DoctypeIdentifierSingleQuoted System),
                _    => go!(error; force_quirks; to BogusDoctype),
            },

            states::AfterDoctypeIdentifier(System) => match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '>' => go!(to Data; emit_doctype),
                _   => go!(error; to BogusDoctype),
            },

            states::BetweenDoctypePublicAndSystemIdentifiers => match get_char!() {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '>'  => go!(to Data; emit_doctype),
                '"'  => go!(clear_doctype_id System; to DoctypeIdentifierDoubleQuoted System),
                '\'' => go!(clear_doctype_id System; to DoctypeIdentifierSingleQuoted System),
                _    => go!(error; force_quirks; to BogusDoctype),
            },

            states::BogusDoctype => match get_char!() {
                '>'  => go!(to Data; emit_doctype),
                _    => (),
            },

            states::BogusComment => match get_char!() {
                '>'  => go!(emit_comment; to Data),
                '\0' => go!(push_comment '\ufffd'),
                c    => go!(push_comment c),
            },

            states::MarkupDeclarationOpen => match () {
                _ if lookahead_and_consume!(2, |s| s == "--")
                    => go!(clear_comment; to CommentStart),
                _ if lookahead_and_consume!(7, |s| s.eq_ignore_ascii_case("doctype"))
                    => go!(to Doctype),
                // FIXME: CDATA, requires "adjusted current node" from tree builder
                // FIXME: 'error' gives wrong message
                _ => go!(error; to BogusComment),
            },

            states::CdataSection
                => fail!("FIXME: state {:?} not implemented", self.state),
        }

        // Most actions above fall through to here.
        // Any case where we didn't make progress has an early "return false".
        true
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
        let CharRef { mut chars, mut num_chars, parse_error } = char_ref;

        if parse_error {
            // FIXME: make this more informative
            self.emit_error(~"Bad character reference");
        }

        if num_chars == 0 {
            chars[0] = '&';
            num_chars = 1;
        }

        for i in range(0, num_chars) {
            let c = chars[i];
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
        // Process all remaining buffered input.
        // FIXME: What if we're doing lookahead, i.e. wait_for = Some(n)?
        self.run();

        // Handle EOF in the char ref sub-tokenizer, if there is one.
        match self.char_ref_tokenizer.take() {
            None => (),
            Some(mut tok) => {
                tok.end_of_file(self);
                self.process_char_ref(tok.get_result());
            }
        }

        loop {
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
                    => go!(error_eof; to Data; emit '<'),

                states::EndTagOpen
                    => go!(error_eof; to Data; emit '<'; emit '/'),

                states::RawLessThanSign(kind)
                    => go!(to RawData kind; emit '<'),

                states::RawEndTagOpen(kind)
                    => go!(to RawData kind; emit '<'; emit '/'),

                states::RawEndTagName(kind)
                    => go!(to RawData kind; emit '<'; emit '/'),

                states::ScriptDataEscapeStart(kind)
                    => go!(to RawData ScriptDataEscaped kind),

                states::ScriptDataEscapeStartDash
                    => go!(to RawData ScriptData),

                states::ScriptDataDoubleEscapeEnd
                    => go!(to RawData ScriptDataEscaped DoubleEscaped),

                states::CommentStart | states::CommentStartDash
                | states::Comment | states::CommentEndDash
                | states::CommentEnd | states::CommentEndBang
                    => go!(error_eof; to Data; emit_comment),

                states::Doctype | states::BeforeDoctypeName
                | states::DoctypeName | states::AfterDoctypeName
                | states::AfterDoctypeKeyword(_) | states::BeforeDoctypeIdentifier(_)
                | states::DoctypeIdentifierDoubleQuoted(_) | states::DoctypeIdentifierSingleQuoted(_)
                | states::AfterDoctypeIdentifier(_) | states::BetweenDoctypePublicAndSystemIdentifiers
                    => go!(error_eof; to Data; create_doctype; force_quirks; emit_doctype),

                states::BogusDoctype
                    => go!(to Data; emit_doctype),

                states::BogusComment
                    => go!(emit_comment; to Data),

                states::MarkupDeclarationOpen
                    => go!(error; to BogusComment),

                states::CdataSection
                    => fail!("FIXME: state {:?} not implemented in EOF", self.state),
            }
        }
    }
}


/// Methods to support the character sub-tokenizer.
/// Putting these in a trait hides the Tokenizer type variables, which makes
/// the sub-tokenizer cleaner.
trait SubTok {
    fn peek(&self) -> Option<char>;
    fn discard_char(&mut self);
    fn unconsume(&mut self, buf: ~str);
}

impl<'sink, Sink: TokenSink> SubTok for Tokenizer<'sink, Sink> {
    fn peek(&self) -> Option<char> {
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

    fn unconsume(&mut self, buf: ~str) {
        self.input_buffers.push_front(buf);
    }
}
