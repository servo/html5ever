// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::{TokenSink, XmlTokenizer};
use crate::data;
use crate::tendril::StrTendril;
use log::debug;
use markup5ever::buffer_queue::BufferQueue;
use std::borrow::Cow::{self, Borrowed};
use std::char::from_u32;

use self::State::*;
pub use self::Status::*;

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

    name_buf_opt: Option<StrTendril>,
    name_match: Option<(u32, u32)>,
    name_len: usize,
}

impl CharRefTokenizer {
    // NB: We assume that we have an additional allowed character iff we're
    // tokenizing in an attribute value.
    pub fn new(addnl_allowed: Option<char>) -> CharRefTokenizer {
        CharRefTokenizer {
            state: Begin,
            addnl_allowed,
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

    fn name_buf(&self) -> &StrTendril {
        self.name_buf_opt
            .as_ref()
            .expect("name_buf missing in named character reference")
    }

    fn name_buf_mut(&mut self) -> &mut StrTendril {
        self.name_buf_opt
            .as_mut()
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
    pub fn step<Sink: TokenSink>(
        &mut self,
        tokenizer: &XmlTokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        if self.result.is_some() {
            return Done;
        }

        debug!("char ref tokenizer stepping in state {:?}", self.state);
        match self.state {
            Begin => self.do_begin(tokenizer, input),
            Octothorpe => self.do_octothorpe(tokenizer, input),
            Numeric(base) => self.do_numeric(tokenizer, base, input),
            NumericSemicolon => self.do_numeric_semicolon(tokenizer, input),
            Named => self.do_named(tokenizer, input),
            BogusName => self.do_bogus_name(tokenizer, input),
        }
    }

    fn do_begin<Sink: TokenSink>(
        &mut self,
        tokenizer: &XmlTokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        match tokenizer.peek(input) {
            Some('\t' | '\n' | '\x0C' | ' ' | '<' | '&') => self.finish_none(),
            Some(c) if Some(c) == self.addnl_allowed => self.finish_none(),
            Some('#') => {
                tokenizer.discard_char(input);
                self.state = Octothorpe;
                Progress
            },
            Some(_) => {
                self.state = Named;
                self.name_buf_opt = Some(StrTendril::new());
                Progress
            },
            None => Stuck,
        }
    }

    fn do_octothorpe<Sink: TokenSink>(
        &mut self,
        tokenizer: &XmlTokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        match tokenizer.peek(input) {
            Some(c @ ('x' | 'X')) => {
                tokenizer.discard_char(input);
                self.hex_marker = Some(c);
                self.state = Numeric(16);
            },
            Some(_) => {
                self.hex_marker = None;
                self.state = Numeric(10);
            },
            None => return Stuck,
        }
        Progress
    }

    fn do_numeric<Sink: TokenSink>(
        &mut self,
        tokenizer: &XmlTokenizer<Sink>,
        base: u32,
        input: &BufferQueue,
    ) -> Status {
        let Some(c) = tokenizer.peek(input) else {
            return Stuck;
        };
        match c.to_digit(base) {
            Some(n) => {
                tokenizer.discard_char(input);
                self.num = self.num.wrapping_mul(base);
                if self.num > 0x10FFFF {
                    // We might overflow, and the character is definitely invalid.
                    // We still parse digits and semicolon, but don't use the result.
                    self.num_too_big = true;
                }
                self.num = self.num.wrapping_add(n);
                self.seen_digit = true;
                Progress
            },

            None if !self.seen_digit => self.unconsume_numeric(tokenizer, input),

            None => {
                self.state = NumericSemicolon;
                Progress
            },
        }
    }

    fn do_numeric_semicolon<Sink: TokenSink>(
        &mut self,
        tokenizer: &XmlTokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        match tokenizer.peek(input) {
            Some(';') => tokenizer.discard_char(input),
            Some(_) => tokenizer.emit_error(Borrowed(
                "Semicolon missing after numeric character reference",
            )),
            None => return Stuck,
        };
        self.finish_numeric(tokenizer)
    }

    fn unconsume_numeric<Sink: TokenSink>(
        &mut self,
        tokenizer: &XmlTokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        let mut unconsume = StrTendril::from_char('#');
        if let Some(c) = self.hex_marker {
            unconsume.push_char(c);
        }

        tokenizer.unconsume(input, unconsume);
        tokenizer.emit_error(Borrowed("Numeric character reference without digits"));
        self.finish_none()
    }

    fn finish_numeric<Sink: TokenSink>(&mut self, tokenizer: &XmlTokenizer<Sink>) -> Status {
        fn conv(n: u32) -> char {
            from_u32(n).expect("invalid char missed by error handling cases")
        }

        let (c, error) = match self.num {
            n if (n > 0x10FFFF) || self.num_too_big => ('\u{fffd}', true),
            0x00 | 0xD800..=0xDFFF => ('\u{fffd}', true),

            0x80..=0x9F => match data::C1_REPLACEMENTS[(self.num - 0x80) as usize] {
                Some(c) => (c, true),
                None => (conv(self.num), true),
            },

            0x01..=0x08 | 0x0B | 0x0D..=0x1F | 0x7F | 0xFDD0..=0xFDEF => (conv(self.num), true),

            n if (n & 0xFFFE) == 0xFFFE => (conv(n), true),

            n => (conv(n), false),
        };

        if error {
            let msg = if tokenizer.opts.exact_errors {
                Cow::from(format!(
                    "Invalid numeric character reference value 0x{:06X}",
                    self.num
                ))
            } else {
                Cow::from("Invalid numeric character reference")
            };
            tokenizer.emit_error(msg);
        }

        self.finish_one(c)
    }

    fn do_named<Sink: TokenSink>(
        &mut self,
        tokenizer: &XmlTokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        let Some(c) = tokenizer.get_char(input) else {
            return Stuck;
        };
        self.name_buf_mut().push_char(c);
        match data::NAMED_ENTITIES.get(&self.name_buf()[..]) {
            // We have either a full match or a prefix of one.
            Some(&m) => {
                if m.0 != 0 {
                    // We have a full match, but there might be a longer one to come.
                    self.name_match = Some(m);
                    self.name_len = self.name_buf().len();
                }
                // Otherwise we just have a prefix match.
                Progress
            },

            // Can't continue the match.
            None => self.finish_named(tokenizer, Some(c), input),
        }
    }

    fn emit_name_error<Sink: TokenSink>(&mut self, tokenizer: &XmlTokenizer<Sink>) {
        let msg = if tokenizer.opts.exact_errors {
            Cow::from(format!("Invalid character reference &{}", self.name_buf()))
        } else {
            Cow::from("Invalid character reference")
        };
        tokenizer.emit_error(msg);
    }

    fn unconsume_name<Sink: TokenSink>(
        &mut self,
        tokenizer: &XmlTokenizer<Sink>,
        input: &BufferQueue,
    ) {
        tokenizer.unconsume(input, self.name_buf_opt.take().unwrap());
    }

    fn finish_named<Sink: TokenSink>(
        &mut self,
        tokenizer: &XmlTokenizer<Sink>,
        end_char: Option<char>,
        input: &BufferQueue,
    ) -> Status {
        match self.name_match {
            None => {
                match end_char {
                    Some(c) if c.is_ascii_alphanumeric() => {
                        // Keep looking for a semicolon, to determine whether
                        // we emit a parse error.
                        self.state = BogusName;
                        return Progress;
                    },

                    // Check length because &; is not a parse error.
                    Some(';') if self.name_buf().len() > 1 => self.emit_name_error(tokenizer),

                    _ => (),
                }
                self.unconsume_name(tokenizer, input);
                self.finish_none()
            },

            Some((c1, c2)) => {
                // We have a complete match, but we may have consumed
                // additional characters into self.name_buf.  Usually
                // at least one, but several in cases like
                //
                //     &not    => match for U+00AC
                //     &noti   => valid prefix for &notin
                //     &notit  => can't continue match

                let name_len = self.name_len;
                assert!(name_len > 0);
                let last_matched = self.name_buf()[name_len - 1..].chars().next().unwrap();

                // There might not be a next character after the match, if
                // we had a full match and then hit EOF.
                let next_after = if name_len == self.name_buf().len() {
                    None
                } else {
                    Some(self.name_buf()[name_len..].chars().next().unwrap())
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
                        tokenizer.emit_error(Borrowed(
                            "Equals sign after character reference in attribute",
                        ));
                        true
                    },
                    (Some(_), _, Some(c)) if c.is_ascii_alphanumeric() => true,
                    _ => {
                        tokenizer.emit_error(Borrowed(
                            "Character reference does not end with semicolon",
                        ));
                        false
                    },
                };

                if unconsume_all {
                    self.unconsume_name(tokenizer, input);
                    self.finish_none()
                } else {
                    tokenizer
                        .unconsume(input, StrTendril::from_slice(&self.name_buf()[name_len..]));
                    self.result = Some(CharRef {
                        chars: [from_u32(c1).unwrap(), from_u32(c2).unwrap()],
                        num_chars: if c2 == 0 { 1 } else { 2 },
                    });
                    Done
                }
            },
        }
    }

    fn do_bogus_name<Sink: TokenSink>(
        &mut self,
        tokenizer: &XmlTokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        let Some(c) = tokenizer.get_char(input) else {
            return Stuck;
        };
        self.name_buf_mut().push_char(c);
        match c {
            _ if c.is_ascii_alphanumeric() => return Progress,
            ';' => self.emit_name_error(tokenizer),
            _ => (),
        }
        self.unconsume_name(tokenizer, input);
        self.finish_none()
    }

    pub fn end_of_file<Sink: TokenSink>(
        &mut self,
        tokenizer: &XmlTokenizer<Sink>,
        input: &BufferQueue,
    ) {
        while self.result.is_none() {
            match self.state {
                Begin => drop(self.finish_none()),

                Numeric(_) if !self.seen_digit => drop(self.unconsume_numeric(tokenizer, input)),

                Numeric(_) | NumericSemicolon => {
                    tokenizer.emit_error(Borrowed("EOF in numeric character reference"));
                    self.finish_numeric(tokenizer);
                },

                Named => drop(self.finish_named(tokenizer, None, input)),

                BogusName => {
                    self.unconsume_name(tokenizer, input);
                    self.finish_none();
                },

                Octothorpe => {
                    tokenizer.unconsume(input, StrTendril::from_slice("#"));
                    tokenizer.emit_error(Borrowed("EOF after '#' in character reference"));
                    self.finish_none();
                },
            }
        }
    }
}
