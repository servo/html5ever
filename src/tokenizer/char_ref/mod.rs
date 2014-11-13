// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::prelude::*;

use super::{TokenizerInner, TokenSink, Span};
use util::single_char::{SingleChar, MayAppendSingleChar};

use util::fast_option::{Uninit, Full, FastOption};
use util::span::ValidatedSpanUtils;
use util::str::is_ascii_alnum;

use core::char::from_u32;
use collections::str::Slice;

use iobuf::{BufSpan, Iobuf, RWIobuf};

pub use self::Status::*;
use self::State::*;

mod data;

//§ tokenizing-character-references
pub struct CharRef {
    /// The resulting character(s)
    pub chars: Span,
}

pub enum Status {
    Stuck,
    Progress,
    Done,
}

#[deriving(Show)]
enum State {
    Begin,
    Octothorpe(SingleChar),
    Numeric(SingleChar /* octothorpe char */, u32), // base
    NumericSemicolon,
    Named,
    BogusName,
}

pub struct CharRefTokenizer {
    state: State,
    addnl_allowed: Option<u8>,
    /// The initial ampersand.
    pub amp: SingleChar,
    result: Option<CharRef>,

    num: u32,
    num_too_big: bool,
    seen_digit: bool,
    hex_marker: Option<SingleChar>,

    name_buf_opt: Option<Span>,
    name_match: Option<data::NamedEntity>,
}

impl CharRefTokenizer {
    // NB: We assume that we have an additional allowed character iff we're
    // tokenizing in an attribute value.
    pub fn new(amp: SingleChar, addnl_allowed: Option<u8>) -> CharRefTokenizer {
        CharRefTokenizer {
            state: Begin,
            addnl_allowed: addnl_allowed,
            amp: amp,
            result: None,
            num: 0,
            num_too_big: false,
            seen_digit: false,
            hex_marker: None,
            name_buf_opt: None,
            name_match: None,
        }
    }

    // A CharRefTokenizer can only tokenize one character reference,
    // so this method consumes the tokenizer.
    pub fn get_result(self) -> Span {
        let CharRef { chars } = self.result.expect("get_result called before done");
        if chars.is_empty() {
            self.amp.into_span()
        } else {
            chars
        }
    }

    fn name_buf<'t>(&'t self) -> &'t Span {
        self.name_buf_opt.as_ref()
            .expect("name_buf missing in named character reference")
    }

    fn name_buf_mut<'t>(&'t mut self) -> &'t mut Span {
        self.name_buf_opt.as_mut()
            .expect("name_buf missing in named character reference")
    }

    fn finish_none(&mut self) -> Status {
        self.result = Some(CharRef {
            chars: BufSpan::new(),
        });
        Done
    }

    fn finish_one(&mut self, chr: SingleChar) -> Status {
        self.result = Some(CharRef {
            chars: chr.into_span(),
        });
        Done
    }
}

impl<Sink: TokenSink> CharRefTokenizer {
    pub fn step(&mut self, tokenizer: &mut TokenizerInner<Sink>) -> Status {
        if self.result.is_some() {
            return Done;
        }

        h5e_debug!("char ref tokenizer stepping in state {}", self.state);
        let octothorpe_char = match self.state {
            Octothorpe(ref c) | Numeric(ref c, _) => Some((*c).clone()),
            _ => None,
        };

        match self.state {
            Begin => self.do_begin(tokenizer),
            Octothorpe(_) => self.do_octothorpe(tokenizer, octothorpe_char.unwrap()),
            Numeric(_, base) => self.do_numeric(tokenizer, octothorpe_char.unwrap(), base),
            NumericSemicolon => self.do_numeric_semicolon(tokenizer),
            Named => self.do_named(tokenizer),
            BogusName => self.do_bogus_name(tokenizer),
        }
    }

    fn do_begin(&mut self, tokenizer: &mut TokenizerInner<Sink>) -> Status {
        let mut c = FastOption::new();

        match tokenizer.peek(&mut c) {
            Uninit => return Stuck,
            Full => {},
        }

        match c.as_ref().as_u8() {
            b'\t' | b'\n' | b'\x0C' | b' ' | b'<' | b'&'
                => self.finish_none(),
            chr if Some(chr) == self.addnl_allowed
                => self.finish_none(),

            b'#' => {
                tokenizer.discard_char();
                self.state = Octothorpe(c.take());
                Progress
            }

            _ => {
                self.state = Named;
                self.name_buf_opt = Some(BufSpan::new());
                Progress
            }
        }
    }

    fn do_octothorpe(&mut self, tokenizer: &mut TokenizerInner<Sink>, octothorpe_char: SingleChar) -> Status {
        let mut c = FastOption::new();

        match tokenizer.peek(&mut c) {
            Uninit => return Stuck,
            Full => {},
        }

        match c.as_ref().as_u8() {
            b'x' | b'X' => {
                tokenizer.discard_char();
                self.hex_marker = Some(c.take());
                self.state = Numeric(octothorpe_char, 16);
            }

            _ => {
                self.hex_marker = None;
                self.state = Numeric(octothorpe_char, 10);
            }
        }
        Progress
    }

    fn do_numeric(&mut self, tokenizer: &mut TokenizerInner<Sink>, octothorpe_char: SingleChar, base: u32) -> Status {
        let mut c = FastOption::new();

        match tokenizer.peek(&mut c) {
            Uninit => return Stuck,
            Full => {},
        }

        match Char::to_digit(c.as_ref().as_u8() as char, base as uint) {
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

            None if !self.seen_digit => self.unconsume_numeric(tokenizer, octothorpe_char),

            None => {
                self.state = NumericSemicolon;
                Progress
            }
        }
    }

    fn do_numeric_semicolon(&mut self, tokenizer: &mut TokenizerInner<Sink>) -> Status {
        let mut c = FastOption::new();

        match tokenizer.peek(&mut c) {
            Uninit => return Stuck,
            Full => {},
        }

        match c.as_ref().as_u8() {
            b';' => tokenizer.discard_char(),
            _    => tokenizer.emit_error(Slice("Semicolon missing after numeric character reference")),
        };

        self.finish_numeric(tokenizer)
    }

    fn unconsume_numeric(&mut self, tokenizer: &mut TokenizerInner<Sink>, octothorpe: SingleChar) -> Status {
        let mut unconsume = octothorpe.into_span();
        match self.hex_marker {
            Some(ref marker) => unconsume.push_sc((*marker).clone()),
            None    => {},
        }

        tokenizer.unconsume(unconsume);
        tokenizer.emit_error(Slice("Numeric character reference without digits"));
        self.finish_none()
    }

    fn finish_numeric(&mut self, tokenizer: &mut TokenizerInner<Sink>) -> Status {
        fn conv(n: u32) -> SingleChar {
            let c = from_u32(n).expect("invalid char missed by error handling cases");
            let b = RWIobuf::new(c.len_utf8());
            unsafe { c.encode_utf8(b.as_mut_window_slice()); }
            SingleChar::new(b.read_only())
        }

        let (c, error) = match self.num {
            n if (n > 0x10FFFF) || self.num_too_big => (SingleChar::unicode_replacement(), true),
            0x00 | 0xD800...0xDFFF => (SingleChar::unicode_replacement(), true),

            0x80...0x9F => match data::lookup_c1_replacement((self.num - 0x80) as uint) {
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

    fn do_named(&mut self, tokenizer: &mut TokenizerInner<Sink>) -> Status {
        let mut c = FastOption::new();

        match tokenizer.get_char(&mut c) {
            Uninit => return Stuck,
            Full => {},
        }

        self.name_buf_mut().push_sc((*c.as_ref()).clone());

        match data::lookup_named_entity(self.name_buf()) {
            // We have either a full match or a prefix of one.
            Some(m) => {
                if m.num_chars > 0 {
                    // We have a full match, but there might be a longer one to come.
                    self.name_match = Some(m);
                }
                // Otherwise, we just have a prefix match.
                Progress
            }
            // Can't continue the match.
            None => {
                let to_finish_with = Some(c.take());
                self.finish_named(tokenizer, to_finish_with)
            },
        }
    }

    fn emit_name_error(&mut self, tokenizer: &mut TokenizerInner<Sink>) {
        let msg = format_if!(tokenizer.opts.exact_errors,
            "Invalid character reference",
            "Invalid character reference &{}", self.name_buf());
        tokenizer.emit_error(msg);
    }

    fn unconsume_name(&mut self, tokenizer: &mut TokenizerInner<Sink>) {
        tokenizer.unconsume(self.name_buf_opt.clone().unwrap());
    }

    fn finish_named(&mut self,
            tokenizer: &mut TokenizerInner<Sink>,
            end_char: Option<SingleChar>) -> Status {
        let result = match self.name_match {
            None => {
                match end_char {
                    Some(ref c) if is_ascii_alnum(c.as_u8() as char) => {
                        // Keep looking for a semicolon, to determine whether
                        // we emit a parse error.
                        self.state = BogusName;
                        return Progress;
                    }
                    // Check length because &; is not a parse error.
                    Some(ref c) if c.as_u8() == b';' && self.name_buf().count_bytes() > 1 =>
                        self.emit_name_error(tokenizer),

                    _ => (),
                }
                self.unconsume_name(tokenizer);
                Ok(self.finish_none())
            }

            Some(ref name_match) => {
                // We have a complete match, but we may have consumed
                // additional characters into self.name_buf.  Usually
                // at least one, but several in cases like
                //
                //     &not    => match for U+00AC
                //     &noti   => valid prefix for &notin
                //     &notit  => can't continue match

                let name_match_byte_len = name_match.key.as_bytes().len() as u32;

                let last_matched = {
                    assert!(name_match_byte_len > 0);
                    // We know the name buf is utf-8, since it is a result of
                    // matching something in our table in data.rs.
                    self.name_buf().iter_bytes().skip(name_match_byte_len as uint - 1).next().unwrap()
                };

                // There might not be a next character after the match, if
                // we had a full match and then hit EOF.
                let next_after = if name_match_byte_len == self.name_buf().count_bytes() {
                    None
                } else {
                    self.name_buf().iter_bytes().skip(name_match_byte_len as uint).next()
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
                    (_, b';', _) => false,
                    (Some(_), _, Some(b'=')) => {
                        tokenizer.emit_error(Slice("Equals sign after character reference in attribute"));
                        true
                    }
                    (Some(_), _, Some(c)) if c < 0x80 && is_ascii_alnum(c as char) => true,
                    _ => {
                        tokenizer.emit_error(Slice("Character reference does not end with semicolon"));
                        false
                    }
                };

                if unconsume_all {
                    Err(())
                } else {
                    tokenizer.unconsume(self.name_buf_opt.clone().unwrap().slice_from(name_match_byte_len));
                    self.result = Some(CharRef {
                        chars: BufSpan::from_buf(name_match.chars.clone()),
                    });
                    Ok(Done)
                }
            }
        };

        match result {
            Ok(status) => status,
            Err(()) => {
                self.unconsume_name(tokenizer);
                self.finish_none()
            }
        }
    }

    fn do_bogus_name(&mut self, tokenizer: &mut TokenizerInner<Sink>) -> Status {
        let mut c = FastOption::new();

        match tokenizer.get_char(&mut c) {
            Uninit => return Stuck,
            Full => {},
        }

        let chr = c.as_ref().as_u8();
        self.name_buf_mut().push_sc(c.take());

        match chr {
            _ if is_ascii_alnum(chr as char) => return Progress,
            b';' => self.emit_name_error(tokenizer),
            _ => ()
        }
        self.unconsume_name(tokenizer);
        self.finish_none()
    }

    pub fn end_of_file(&mut self, tokenizer: &mut TokenizerInner<Sink>) {
        while self.result.is_none() {
            let octothorpe_char = match self.state {
                Numeric(ref c, _) | Octothorpe(ref c) => Some((*c).clone()),
                _ => None,
            };

            match self.state {
                Begin => drop(self.finish_none()),

                Numeric(_, _) if !self.seen_digit
                    => drop(self.unconsume_numeric(tokenizer, octothorpe_char.unwrap())),

                Numeric(_, _) | NumericSemicolon => {
                    tokenizer.emit_error(Slice("EOF in numeric character reference"));
                    self.finish_numeric(tokenizer);
                }

                Named => drop(self.finish_named(tokenizer, None)),

                BogusName => {
                    self.unconsume_name(tokenizer);
                    self.finish_none();
                }

                Octothorpe(_) => {
                    tokenizer.unconsume(octothorpe_char.unwrap().into_span());
                    tokenizer.emit_error(Slice("EOF after '#' in character reference"));
                    self.finish_none();
                }
            }
        }
    }
}
