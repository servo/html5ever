// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::{Tokenizer, TokenSink};

use util::str::{is_ascii_alnum, empty_str};
use util::str::{is_xml_namechar};

use std::char::from_u32;
use std::borrow::Cow::Borrowed;

pub use self::Status::*;
use self::State::*;

use super::{XTokenSink, XmlTokenizer};
pub use self::XRef::*;
use self::XState::*;

mod data;

//§ tokenizing-character-references
pub struct CharRef {
    /// The resulting character(s)
    pub chars: [char; 2],

    /// How many slots in `chars` are valid?
    pub num_chars: u8,
}

pub enum Status {
    Stuck,
    Progress,
    Done,
}

#[derive(Debug)]
enum State {
    Begin,
    Octothorpe,
    Numeric(u32), // base
    NumericSemicolon,
    Named,
    BogusName,
}

pub struct CharRefTokenizer {
    state: State,
    addnl_allowed: Option<char>,
    result: Option<CharRef>,

    num: u32,
    num_too_big: bool,
    seen_digit: bool,
    hex_marker: Option<char>,

    name_buf_opt: Option<String>,
    name_match: Option<&'static [u32; 2]>,
    name_len: usize,
}

impl CharRefTokenizer {
    // NB: We assume that we have an additional allowed character iff we're
    // tokenizing in an attribute value.
    pub fn new(addnl_allowed: Option<char>) -> CharRefTokenizer {
        CharRefTokenizer {
            state: Begin,
            addnl_allowed: addnl_allowed,
            result: None,
            num: 0,
            num_too_big: false,
            seen_digit: false,
            hex_marker: None,
            name_buf_opt: None,
            name_match: None,
            name_len: 0,
        }
    }

    // A CharRefTokenizer can only tokenize one character reference,
    // so this method consumes the tokenizer.
    pub fn get_result(self) -> CharRef {
        self.result.expect("get_result called before done")
    }

    fn name_buf<'t>(&'t self) -> &'t String {
        self.name_buf_opt.as_ref()
            .expect("name_buf missing in named character reference")
    }

    fn name_buf_mut<'t>(&'t mut self) -> &'t mut String {
        self.name_buf_opt.as_mut()
            .expect("name_buf missing in named character reference")
    }

    fn finish_none(&mut self) -> Status {
        self.result = Some(CharRef {
            chars: ['\0', '\0'],
            num_chars: 0,
        });
        Done
    }

    fn finish_one(&mut self, c: char) -> Status {
        self.result = Some(CharRef {
            chars: [c, '\0'],
            num_chars: 1,
        });
        Done
    }
}

impl CharRefTokenizer {
    pub fn step<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>) -> Status {
        if self.result.is_some() {
            return Done;
        }

        debug!("char ref tokenizer stepping in state {:?}", self.state);
        match self.state {
            Begin => self.do_begin(tokenizer),
            Octothorpe => self.do_octothorpe(tokenizer),
            Numeric(base) => self.do_numeric(tokenizer, base),
            NumericSemicolon => self.do_numeric_semicolon(tokenizer),
            Named => self.do_named(tokenizer),
            BogusName => self.do_bogus_name(tokenizer),
        }
    }

    fn do_begin<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>) -> Status {
        match unwrap_or_return!(tokenizer.peek(), Stuck) {
            '\t' | '\n' | '\x0C' | ' ' | '<' | '&'
                => self.finish_none(),
            c if Some(c) == self.addnl_allowed
                => self.finish_none(),

            '#' => {
                tokenizer.discard_char();
                self.state = Octothorpe;
                Progress
            }

            _ => {
                self.state = Named;
                self.name_buf_opt = Some(empty_str());
                Progress
            }
        }
    }

    fn do_octothorpe<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>) -> Status {
        let c = unwrap_or_return!(tokenizer.peek(), Stuck);
        match c {
            'x' | 'X' => {
                tokenizer.discard_char();
                self.hex_marker = Some(c);
                self.state = Numeric(16);
            }

            _ => {
                self.hex_marker = None;
                self.state = Numeric(10);
            }
        }
        Progress
    }

    fn do_numeric<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>, base: u32) -> Status {
        let c = unwrap_or_return!(tokenizer.peek(), Stuck);
        match c.to_digit(base) {
            Some(n) => {
                tokenizer.discard_char();
                self.num = self.num.wrapping_mul(base);
                if self.num > 0x10FFFF {
                    // We might overflow, and the character is definitely invalid.
                    // We still parse digits and semicolon, but don't use the result.
                    self.num_too_big = true;
                }
                self.num = self.num.wrapping_add(n);
                self.seen_digit = true;
                Progress
            }

            None if !self.seen_digit => self.unconsume_numeric(tokenizer),

            None => {
                self.state = NumericSemicolon;
                Progress
            }
        }
    }

    fn do_numeric_semicolon<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>) -> Status {
        match unwrap_or_return!(tokenizer.peek(), Stuck) {
            ';' => tokenizer.discard_char(),
            _   => tokenizer.emit_error(Borrowed("Semicolon missing after numeric character reference")),
        };
        self.finish_numeric(tokenizer)
    }

    fn unconsume_numeric<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>) -> Status {
        let mut unconsume = String::from_str("#");
        match self.hex_marker {
            Some(c) => unconsume.push(c),
            None => (),
        }

        tokenizer.unconsume(unconsume);
        tokenizer.emit_error(Borrowed("Numeric character reference without digits"));
        self.finish_none()
    }

    fn finish_numeric<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>) -> Status {
        fn conv(n: u32) -> char {
            from_u32(n).expect("invalid char missed by error handling cases")
        }

        let (c, error) = match self.num {
            n if (n > 0x10FFFF) || self.num_too_big => ('\u{fffd}', true),
            0x00 | 0xD800...0xDFFF => ('\u{fffd}', true),

            0x80...0x9F => match data::C1_REPLACEMENTS[(self.num - 0x80) as usize] {
                Some(c) => (c, true),
                None => (conv(self.num), true),
            },

            0x01...0x08 | 0x0B | 0x0D...0x1F | 0x7F | 0xFDD0...0xFDEF
                => (conv(self.num), true),

            n if (n & 0xFFFE) == 0xFFFE
                => (conv(n), true),

            n => (conv(n), false),
        };

        if error {
            let msg = format_if!(tokenizer.opts.exact_errors,
                "Invalid numeric character reference",
                "Invalid numeric character reference value 0x{:06X}", self.num);
            tokenizer.emit_error(msg);
        }

        self.finish_one(c)
    }

    fn do_named<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>) -> Status {
        let c = unwrap_or_return!(tokenizer.get_char(), Stuck);
        self.name_buf_mut().push(c);
        match data::NAMED_ENTITIES.get(&self.name_buf()[..]) {
            // We have either a full match or a prefix of one.
            Some(m) => {
                if m[0] != 0 {
                    // We have a full match, but there might be a longer one to come.
                    self.name_match = Some(m);
                    self.name_len = self.name_buf().len();
                }
                // Otherwise we just have a prefix match.
                Progress
            }

            // Can't continue the match.
            None => self.finish_named(tokenizer, Some(c)),
        }
    }

    fn emit_name_error<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>) {
        let msg = format_if!(tokenizer.opts.exact_errors,
            "Invalid character reference",
            "Invalid character reference &{}", self.name_buf());
        tokenizer.emit_error(msg);
    }

    fn unconsume_name<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>) {
        tokenizer.unconsume(self.name_buf_opt.take().unwrap());
    }

    fn finish_named<Sink: TokenSink>(&mut self,
            tokenizer: &mut Tokenizer<Sink>,
            end_char: Option<char>) -> Status {
        match self.name_match {
            None => {
                match end_char {
                    Some(c) if is_ascii_alnum(c) => {
                        // Keep looking for a semicolon, to determine whether
                        // we emit a parse error.
                        self.state = BogusName;
                        return Progress;
                    }

                    // Check length because &; is not a parse error.
                    Some(';') if self.name_buf().len() > 1
                        => self.emit_name_error(tokenizer),

                    _ => (),
                }
                self.unconsume_name(tokenizer);
                self.finish_none()
            }

            Some(&[c1, c2]) => {
                // We have a complete match, but we may have consumed
                // additional characters into self.name_buf.  Usually
                // at least one, but several in cases like
                //
                //     &not    => match for U+00AC
                //     &noti   => valid prefix for &notin
                //     &notit  => can't continue match

                let name_len = self.name_len;
                assert!(name_len > 0);
                let last_matched = self.name_buf().char_at(name_len-1);

                // There might not be a next character after the match, if
                // we had a full match and then hit EOF.
                let next_after = if name_len == self.name_buf().len() {
                    None
                } else {
                    Some(self.name_buf().char_at(name_len))
                };

                // "If the character reference is being consumed as part of an
                // attribute, and the last character matched is not a U+003B
                // SEMICOLON character (;), and the next character is either a
                // U+003D EQUALS SIGN character (=) or an alphanumeric ASCII
                // character, then, for historical reasons, all the characters
                // that were matched after the U+0026 AMPERSAND character (&)
                // must be unconsumed, and nothing is returned. However, if
                // this next character is in fact a U+003D EQUALS SIGN
                // character (=), then this is a parse error"

                let unconsume_all = match (self.addnl_allowed, last_matched, next_after) {
                    (_, ';', _) => false,
                    (Some(_), _, Some('=')) => {
                        tokenizer.emit_error(Borrowed("Equals sign after character reference in attribute"));
                        true
                    }
                    (Some(_), _, Some(c)) if is_ascii_alnum(c) => true,
                    _ => {
                        tokenizer.emit_error(Borrowed("Character reference does not end with semicolon"));
                        false
                    }
                };

                if unconsume_all {
                    self.unconsume_name(tokenizer);
                    self.finish_none()
                } else {
                    tokenizer.unconsume(String::from_str(&self.name_buf()[name_len..]));
                    self.result = Some(CharRef {
                        chars: [from_u32(c1).unwrap(), from_u32(c2).unwrap()],
                        num_chars: if c2 == 0 { 1 } else { 2 },
                    });
                    Done
                }
            }
        }
    }

    fn do_bogus_name<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>) -> Status {
        let c = unwrap_or_return!(tokenizer.get_char(), Stuck);
        self.name_buf_mut().push(c);
        match c {
            _ if is_ascii_alnum(c) => return Progress,
            ';' => self.emit_name_error(tokenizer),
            _ => ()
        }
        self.unconsume_name(tokenizer);
        self.finish_none()
    }

    pub fn end_of_file<Sink: TokenSink>(&mut self, tokenizer: &mut Tokenizer<Sink>) {
        while self.result.is_none() {
            match self.state {
                Begin => drop(self.finish_none()),

                Numeric(_) if !self.seen_digit
                    => drop(self.unconsume_numeric(tokenizer)),

                Numeric(_) | NumericSemicolon => {
                    tokenizer.emit_error(Borrowed("EOF in numeric character reference"));
                    self.finish_numeric(tokenizer);
                }

                Named => drop(self.finish_named(tokenizer, None)),

                BogusName => {
                    self.unconsume_name(tokenizer);
                    self.finish_none();
                }

                Octothorpe => {
                    tokenizer.unconsume(String::from_str("#"));
                    tokenizer.emit_error(Borrowed("EOF after '#' in character reference"));
                    self.finish_none();
                }
            }
        }
    }
}
#[derive(Debug)]
enum XState {
    XBegin,
    XNumeric(u32),
    XNumericSemicolon,
    XReference,
    XOctothorpe,
}

pub enum XRef {
    NamedXRef(String),
    CharXData(String),
    NoReturn,
}

pub struct XCharRefTokenizer {
    state: XState,
    result: Option<XRef>,
    hex_marker: Option<char>,

    num: u32,
    num_too_big: bool,

    seen_digit: bool,
    name_buf_opt: Option<String>,
}
impl XCharRefTokenizer {

    // NB: We assume that we have an additional allowed character iff we're
    // tokenizing in an attribute value.
    pub fn new() -> XCharRefTokenizer {
        XCharRefTokenizer {
            state: XState::XBegin,
            result: None,
            hex_marker: None,
            num: 0,
            num_too_big: false,
            seen_digit: false,
            name_buf_opt: None,
        }
    }

    // A CharRefTokenizer can only tokenize one character reference,
    // so this method consumes the tokenizer.
    pub fn get_result(self) -> XRef {
        self.result.expect("get_result called before done")
    }

    fn name_buf_mut<'t>(&'t mut self) -> &'t mut String {
        self.name_buf_opt.as_mut()
            .expect("name_buf missing in named character reference")
    }

    fn name_buf<'t>(&'t self) -> &'t String {
        self.name_buf_opt.as_ref()
            .expect("name_buf missing in named character reference")
    }
}



impl XCharRefTokenizer {

    pub fn step<Sink: XTokenSink>(
        &mut self,
        tokenizer: &mut XmlTokenizer<Sink>
        ) -> Status {

        if self.result.is_some() {
            return Done;
        }

        debug!("Xml char ref tokenizer stepping in state {:?}", self.state);
        match self.state {
            XBegin => self.do_begin(tokenizer),
            XNumeric(base) => self.do_numeric(tokenizer, base),
            XNumericSemicolon => self.do_numeric_semicolon(tokenizer),
            XReference => self.do_reference(tokenizer),
            XOctothorpe => self.do_octothorpe(tokenizer),
        }
    }

    fn do_begin<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        match unwrap_or_return!(tokenizer.peek(), Stuck) {
            '\t' | '\n' | '\x0C' | ' ' | '<' | '&' | '%'
                => self.finish_none(),

            '#' => {
                tokenizer.discard_char();
                self.state = XState::XOctothorpe;
                Progress
            }

            _ => {
                self.state = XState::XReference;
                self.name_buf_opt = Some(empty_str());
                Progress
            }
        }
    }

    fn do_octothorpe<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        let c = unwrap_or_return!(tokenizer.peek(), Stuck);
        match c {
            'x' | 'X' => {
                tokenizer.discard_char();
                self.hex_marker = Some(c);
                self.state = XNumeric(16);
            }

            _ => {
                self.hex_marker = None;
                self.state = XNumeric(10);
            }
        }
        Progress
    }


    fn do_numeric<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>, base: u32) -> Status {
        let c = unwrap_or_return!(tokenizer.peek(), Stuck);
        match c.to_digit(base as u32) {
            Some(n) => {
                tokenizer.discard_char();
                self.num *= base;
                if self.num > 0x10FFFF {
                    // We might overflow, and the character is definitely invalid.
                    // We still parse digits and semicolon, but don't use the result.
                    self.num_too_big = true;
                }
                self.num += n as u32;
                self.seen_digit = true;
                Progress
            }

            None if !self.seen_digit => self.unconsume_numeric(tokenizer),

            None => {
                self.state = XNumericSemicolon;
                Progress
            }
        }
    }

    fn do_numeric_semicolon<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        match unwrap_or_return!(tokenizer.peek(), Stuck) {
            ';' => tokenizer.discard_char(),
            _   => tokenizer.emit_error(Borrowed("Semicolon missing after numeric character reference")),
        };
        self.finish_numeric(tokenizer)
    }

    fn do_reference<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        let c = unwrap_or_return!(tokenizer.get_char(), Stuck);
        if is_xml_namechar(&c) {
            self.name_buf_mut().push(c);
            Progress
        } else if  c == ';' {
            self.finish_reference(tokenizer)
        } else {
            tokenizer.unconsume(c.to_string());
            let temp = self.name_buf().clone();
            self.finish_text(temp)
        }

    }

    pub fn end_of_file<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) {


        while self.result.is_none() {
            match self.state {
                XBegin => { self.finish_none(); },

                XNumeric(_) if !self.seen_digit
                    => { self.unconsume_numeric(tokenizer); },

                XNumeric(_) | XState::XNumericSemicolon => {
                    tokenizer.emit_error(Borrowed("EOF in numeric character reference"));
                    self.finish_numeric(tokenizer);
                },

                XReference => {
                    tokenizer.emit_error(Borrowed("EOF in reference"));
                    self.finish_reference(tokenizer);
                },

                XOctothorpe => {
                    tokenizer.emit_error(Borrowed("EOF after '#' in character reference"));
                    self.finish_text("#".to_string());
                },
            }
        }
    }

    fn finish_none(&mut self) -> Status {
        self.result = Some(NoReturn);
        Done
    }

    fn finish_text(&mut self, text: String) -> Status {
        self.result = Some(CharXData(text));
        Done
    }

    fn finish_reference<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) -> Status {

        use std::mem::replace;

        match self.name_buf_opt {
            Some(ref mut c) if c.len() > 0 => {
                self.result = Some(NamedXRef(replace(c, String::new())));
            },
            _ => {
                tokenizer.emit_error(Borrowed("empty reference"));
                self.result = Some(NoReturn);
            }
        };
        Done
    }

    fn finish_numeric<Sink: XTokenSink>(&mut self, tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        fn conv(n: u32) -> char {
            from_u32(n).expect("invalid char missed by error handling cases")
        }

        let (c, error) = match self.num {
            n if (n > 0x10FFFF) || self.num_too_big => ('\u{fffd}', true),
            n => (conv(n), false),
        };

        if error {
            let msg = format_if!(tokenizer.opts.exact_errors,
                "Invalid numeric character reference",
                "Invalid numeric character reference value 0x{:06X}", self.num);
            tokenizer.emit_error(msg);
        }
        self.result = Some(CharXData(c.to_string()));
        Done
    }

    fn unconsume_numeric<Sink: XTokenSink>(&mut self, tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        let mut unconsume = String::from_str("#");
        match self.hex_marker {
            Some(c) => unconsume.push(c),
            None => (),
        }


        tokenizer.emit_error(Borrowed("Numeric character reference without digits"));
        self.finish_text(unconsume)
    }
}
