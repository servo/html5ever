/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use super::SubTok;

use std::char::{to_digit, from_u32};

mod data;

pub struct CharRef {
    chars: [char, ..2],
    num_chars: u8,
    parse_error: bool,
}

pub enum Status {
    Stuck,
    Progress,
    Done,
}

enum State {
    Begin,
    Octothorpe,
    Numeric(u32), // base
    NumericSemicolon,
    Named,
}

pub struct CharRefTokenizer {
    priv state: State,
    priv addnl_allowed: Option<char>,
    priv result: Option<CharRef>,

    priv num: u32,
    priv num_too_big: bool,
    priv seen_digit: bool,
    priv hex_marker: Option<char>,

    priv name_buf: Option<~str>,
    priv last_name_match: Option<&'static [char, ..2]>,
    priv last_name_char: char,
}

impl CharRefTokenizer {
    pub fn new(addnl_allowed: Option<char>) -> CharRefTokenizer {
        CharRefTokenizer {
            state: Begin,
            addnl_allowed: addnl_allowed,
            result: None,
            num: 0,
            num_too_big: false,
            seen_digit: false,
            hex_marker: None,
            name_buf: None,
            last_name_match: None,
            last_name_char: '\0',
        }
    }

    pub fn step<T: SubTok>(&mut self, tokenizer: &mut T) -> Status {
        if self.result.is_some() {
            return Done;
        }

        match self.state {
            Begin => self.do_begin(tokenizer),
            Octothorpe => self.do_octothorpe(tokenizer),
            Numeric(base) => self.do_numeric(tokenizer, base),
            NumericSemicolon => self.do_numeric_semicolon(tokenizer),
            Named => self.do_named(tokenizer),
        }
    }

    // A CharRefTokenizer can only tokenize one character reference,
    // so this method consumes the tokenizer.
    pub fn get_result(self) -> CharRef {
        self.result.expect("get_result called before done")
    }

    fn finish_none(&mut self, error: bool) -> Status {
        self.result = Some(CharRef {
            chars: ['\0', '\0'],
            num_chars: 0,
            parse_error: error,
        });
        Done
    }

    fn finish_one(&mut self, c: char, error: bool) -> Status {
        self.result = Some(CharRef {
            chars: [c, '\0'],
            num_chars: 1,
            parse_error: error,
        });
        Done
    }

    fn do_begin<T: SubTok>(&mut self, tokenizer: &mut T) -> Status {
        match unwrap_or_return!(tokenizer.peek(), Stuck) {
            '\t' | '\n' | '\x0C' | ' ' | '<' | '&'
                => self.finish_none(false),
            c if Some(c) == self.addnl_allowed
                => self.finish_none(false),

            '#' => {
                tokenizer.discard_char();
                self.state = Octothorpe;
                Progress
            }

            _ => {
                self.state = Named;
                self.name_buf = Some(~"");
                Progress
            }
        }
    }

    fn do_octothorpe<T: SubTok>(&mut self, tokenizer: &mut T) -> Status {
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

    fn do_numeric<T: SubTok>(&mut self, tokenizer: &mut T, base: u32) -> Status {
        let c = unwrap_or_return!(tokenizer.peek(), Stuck);
        match to_digit(c, base as uint) {
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

            None if !self.seen_digit => {
                let mut unconsume = ~"#";
                match self.hex_marker {
                    Some(c) => unconsume.push_char(c),
                    None => (),
                }

                tokenizer.unconsume(unconsume);
                self.finish_none(true)
            }

            None => {
                self.state = NumericSemicolon;
                Progress
            }
        }
    }

    fn do_numeric_semicolon<T: SubTok>(&mut self, tokenizer: &mut T) -> Status {
        fn conv(n: u32) -> char {
            from_u32(n).expect("invalid char missed by error handling cases")
        }

        let semi_missing = match unwrap_or_return!(tokenizer.peek(), Stuck) {
            ';' => { tokenizer.discard_char(); false }
            _   => true
        };

        match self.num {
            n if (n > 0x10FFFF) || self.num_too_big => self.finish_one('\ufffd', true),
            0x00 | 0xD800..0xDFFF => self.finish_one('\ufffd', true),

            0x80..0x9F => match data::c1_replacements[self.num - 0x80] {
                Some(c) => self.finish_one(c, true),
                None => self.finish_one(conv(self.num), semi_missing),
            },

            0x01..0x08 | 0x0D..0x1F | 0x7F..0x9F | 0xFDD0..0xFDEF | 0x0B
                => self.finish_one(conv(self.num), true),

            n if (n & 0xFFFE) == 0xFFFE
                => self.finish_one(conv(n), true),

            n => self.finish_one(conv(n), semi_missing),
        }
    }

    fn do_named<T: SubTok>(&mut self, tokenizer: &mut T) -> Status {
        let c = unwrap_or_return!(tokenizer.peek(), Stuck);
        self.name_buf.get_mut_ref().push_char(c);
        match data::named_entities.find(&self.name_buf.get_ref().as_slice()) {
            Some(m) => {
                // The buffer matches an entity, or a prefix of an entity.  In
                // the latter case, both chars in last_name_match are \0.
                tokenizer.discard_char();
                self.last_name_match = Some(m);
                self.last_name_char = c;
                Progress
            }

            // Can't continue the match.
            None => match self.last_name_match {
                None | Some(&['\0', _]) => {
                    // We matched nothing, or a prefix only.
                    //
                    // FIXME: "if the characters after the U+0026 AMPERSAND
                    // character (&) consist of a sequence of one or more
                    // alphanumeric ASCII characters followed by a U+003B
                    // SEMICOLON character (;), then this is a parse error".

                    tokenizer.discard_char();
                    tokenizer.unconsume(self.name_buf.take_unwrap());
                    self.finish_none(false)
                }
                Some(m) => {
                    // We have a complete match.
                    self.result = Some(CharRef {
                        chars: *m,
                        num_chars: if m[1] == '\0' { 1 } else { 2 },
                        parse_error: self.last_name_char != ';',
                    });
                    Done
                }
            }
        }
    }
}
