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
use std::mem;

use markup5ever::named_entities::{
    format_name_error, CharRef, NamedReferenceTokenizationResult, NamedReferenceTokenizerState,
};

use self::State::*;
pub use self::Status::*;

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
    Named(NamedReferenceTokenizerState),
    BogusName(StrTendril),
}

pub struct CharRefTokenizer {
    state: State,
    addnl_allowed: Option<char>,
    result: Option<CharRef>,

    num: u32,
    num_too_big: bool,
    seen_digit: bool,
    hex_marker: Option<char>,
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
        }
    }

    // A CharRefTokenizer can only tokenize one character reference,
    // so this method consumes the tokenizer.
    pub fn get_result(self) -> CharRef {
        self.result.expect("get_result called before done")
    }

    fn finish_none(&mut self) -> Status {
        self.result = Some(CharRef::EMPTY);
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
            Named(ref mut named_tokenizer) => loop {
                let Some(c) = tokenizer.peek(input) else {
                    return Status::Stuck;
                };
                tokenizer.discard_char(input);

                match named_tokenizer.feed_character(c, input, |error| tokenizer.emit_error(error))
                {
                    NamedReferenceTokenizationResult::Success { reference } => {
                        self.result = Some(reference);
                        return Status::Done;
                    },
                    NamedReferenceTokenizationResult::Failed(characters) => {
                        self.state = State::BogusName(characters);
                        return Status::Progress;
                    },
                    NamedReferenceTokenizationResult::Continue => {},
                }
            },
            State::BogusName(ref mut invalid_name) => {
                let Some(c) = tokenizer.peek(input) else {
                    return Status::Stuck;
                };
                tokenizer.discard_char(input);
                invalid_name.push_char(c);
                match c {
                    _ if c.is_ascii_alphanumeric() => return Status::Progress,
                    ';' => {
                        tokenizer.emit_error(Cow::from(format_name_error(invalid_name.clone())));
                    },
                    _ => (),
                }
                input.push_front(mem::take(invalid_name));
                self.result = Some(CharRef::EMPTY);
                Status::Done
            },
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
                self.state = Named(NamedReferenceTokenizerState::new(
                    self.addnl_allowed.is_some(),
                ));
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

                Named(ref mut state) => {
                    let character_reference = state
                        .notify_end_of_file(|error| tokenizer.emit_error(error), input)
                        .unwrap_or(CharRef::EMPTY);
                    self.result = Some(character_reference);
                },

                BogusName(ref mut bogus_name) => {
                    input.push_front(bogus_name.clone());
                    if bogus_name.ends_with(';') {
                        tokenizer.emit_error(Cow::from(format_name_error(mem::take(bogus_name))));
                    }
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
