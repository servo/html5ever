// Copyright 2014-2025 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod codegen;
mod named;

use super::{TokenSink, Tokenizer};
use crate::buffer_queue::BufferQueue;
use crate::data;
use crate::tendril::StrTendril;
use crate::tokenizer::char_ref::named::emit_name_error;
use named::NamedReferenceTokenizerState;

use log::debug;
use std::borrow::Cow::{self, Borrowed};
use std::char::from_u32;
use std::mem;

#[derive(Clone, Copy, Debug)]
pub(super) struct CharRef {
    /// The resulting character(s)
    pub(super) chars: [char; 2],

    /// How many slots in `chars` are valid?
    pub(super) num_chars: u8,
}

pub(super) enum Status {
    Stuck,
    Progress,
    Done(CharRef),
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

pub(super) struct CharRefTokenizer {
    state: State,
    is_consumed_in_attribute: bool,

    num: u32,
    num_too_big: bool,
    seen_digit: bool,
    hex_marker: Option<char>,
}

impl CharRef {
    const EMPTY: CharRef = CharRef {
        chars: ['\0', '\0'],
        num_chars: 0,
    };
}

impl CharRefTokenizer {
    pub(super) fn new(is_consumed_in_attribute: bool) -> CharRefTokenizer {
        CharRefTokenizer {
            is_consumed_in_attribute,
            state: State::Begin,
            num: 0,
            num_too_big: false,
            seen_digit: false,
            hex_marker: None,
        }
    }

    fn finish_one(&mut self, c: char) -> Status {
        Status::Done(CharRef {
            chars: [c, '\0'],
            num_chars: 1,
        })
    }
}

impl CharRefTokenizer {
    pub(super) fn step<Sink: TokenSink>(
        &mut self,
        tokenizer: &Tokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        debug!("char ref tokenizer stepping in state {:?}", self.state);
        match self.state {
            State::Begin => self.do_begin(tokenizer, input),
            State::Octothorpe => self.do_octothorpe(tokenizer, input),
            State::Numeric(base) => self.do_numeric(tokenizer, input, base),
            State::NumericSemicolon => self.do_numeric_semicolon(tokenizer, input),
            State::Named(ref mut named_tokenizer) => match named_tokenizer.step(tokenizer, input) {
                Ok(status) => status,
                Err(error) => {
                    self.state = State::BogusName(error);
                    Status::Progress
                },
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
                        emit_name_error(invalid_name.clone(), tokenizer);
                    },
                    _ => (),
                }
                input.push_front(mem::take(invalid_name));
                Status::Done(CharRef::EMPTY)
            },
        }
    }

    fn do_begin<Sink: TokenSink>(
        &mut self,
        tokenizer: &Tokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        match tokenizer.peek(input) {
            Some('a'..='z' | 'A'..='Z' | '0'..='9') => {
                self.state = State::Named(NamedReferenceTokenizerState::new(
                    self.is_consumed_in_attribute,
                ));
                Status::Progress
            },
            Some('#') => {
                tokenizer.discard_char(input);
                self.state = State::Octothorpe;
                Status::Progress
            },
            Some(_) => Status::Done(CharRef::EMPTY),
            None => Status::Stuck,
        }
    }

    fn do_octothorpe<Sink: TokenSink>(
        &mut self,
        tokenizer: &Tokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        match tokenizer.peek(input) {
            Some(c @ ('x' | 'X')) => {
                tokenizer.discard_char(input);
                self.hex_marker = Some(c);
                self.state = State::Numeric(16);
            },
            Some(_) => {
                self.hex_marker = None;
                self.state = State::Numeric(10);
            },
            None => return Status::Stuck,
        }
        Status::Progress
    }

    fn do_numeric<Sink: TokenSink>(
        &mut self,
        tokenizer: &Tokenizer<Sink>,
        input: &BufferQueue,
        base: u32,
    ) -> Status {
        let Some(c) = tokenizer.peek(input) else {
            return Status::Stuck;
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
                Status::Progress
            },

            None if !self.seen_digit => self.unconsume_numeric(tokenizer, input),

            None => {
                self.state = State::NumericSemicolon;
                Status::Progress
            },
        }
    }

    fn do_numeric_semicolon<Sink: TokenSink>(
        &mut self,
        tokenizer: &Tokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        match tokenizer.peek(input) {
            Some(';') => tokenizer.discard_char(input),
            Some(_) => tokenizer.emit_error(Borrowed(
                "Semicolon missing after numeric character reference",
            )),
            None => return Status::Stuck,
        };
        self.finish_numeric(tokenizer)
    }

    fn unconsume_numeric<Sink: TokenSink>(
        &mut self,
        tokenizer: &Tokenizer<Sink>,
        input: &BufferQueue,
    ) -> Status {
        let mut unconsume = StrTendril::from_char('#');
        if let Some(c) = self.hex_marker {
            unconsume.push_char(c)
        }

        input.push_front(unconsume);
        tokenizer.emit_error(Borrowed("Numeric character reference without digits"));
        Status::Done(CharRef::EMPTY)
    }

    fn finish_numeric<Sink: TokenSink>(&mut self, tokenizer: &Tokenizer<Sink>) -> Status {
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

    pub(super) fn end_of_file<Sink: TokenSink>(
        &mut self,
        tokenizer: &Tokenizer<Sink>,
        input: &BufferQueue,
    ) -> CharRef {
        loop {
            let status = match &mut self.state {
                State::Begin => Status::Done(CharRef::EMPTY),
                State::Numeric(_) if !self.seen_digit => self.unconsume_numeric(tokenizer, input),
                State::Numeric(_) | State::NumericSemicolon => {
                    tokenizer.emit_error(Borrowed("EOF in numeric character reference"));
                    self.finish_numeric(tokenizer)
                },
                State::Named(state) => {
                    return state
                        .notify_end_of_file(tokenizer, input)
                        .unwrap_or(CharRef::EMPTY)
                },
                State::Octothorpe => {
                    input.push_front(StrTendril::from_slice("#"));
                    tokenizer.emit_error(Borrowed("EOF after '#' in character reference"));
                    Status::Done(CharRef::EMPTY)
                },
                State::BogusName(bogus_name) => {
                    input.push_front(bogus_name.clone());
                    if bogus_name.ends_with(';') {
                        emit_name_error(mem::take(bogus_name), tokenizer);
                    }
                    Status::Done(CharRef::EMPTY)
                },
            };

            match status {
                Status::Done(char_ref) => {
                    return char_ref;
                },
                Status::Stuck => {
                    return CharRef::EMPTY;
                },
                Status::Progress => {},
            }
        }
    }
}
