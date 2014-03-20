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

    priv name_buf_opt: Option<~str>,
    priv name_match: Option<&'static [char, ..2]>,
    priv name_len: uint,
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
            name_buf_opt: None,
            name_match: None,
            name_len: 0,
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

    fn name_buf<'t>(&'t mut self) -> &'t mut ~str {
        self.name_buf_opt.as_mut()
            .expect("name_buf missing in named character reference")
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
                self.name_buf_opt = Some(~"");
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
                None => self.finish_one(conv(self.num), true),
            },

            0x01..0x08 | 0x0B | 0x0D..0x1F | 0x7F | 0xFDD0..0xFDEF
                => self.finish_one(conv(self.num), true),

            n if (n & 0xFFFE) == 0xFFFE
                => self.finish_one(conv(n), true),

            n => self.finish_one(conv(n), semi_missing),
        }
    }

    fn do_named<T: SubTok>(&mut self, tokenizer: &mut T) -> Status {
        let c = unwrap_or_return!(tokenizer.peek(), Stuck);
        tokenizer.discard_char();
        self.name_buf().push_char(c);
        match data::named_entities.find(&self.name_buf().as_slice()) {
            // We have either a full match or a prefix of one.
            Some(m) => {
                if m[0] != '\0' {
                    // We have a full match, but there might be a longer one to come.
                    self.name_match = Some(m);
                    self.name_len = self.name_buf().len();
                }
                // Otherwise we just have a prefix match.
                Progress
            }

            // Can't continue the match.
            None => match self.name_match {
                None => {
                    // FIXME: "if the characters after the U+0026 AMPERSAND
                    // character (&) consist of a sequence of one or more
                    // alphanumeric ASCII characters followed by a U+003B
                    // SEMICOLON character (;), then this is a parse error".

                    tokenizer.unconsume(self.name_buf_opt.take_unwrap());
                    self.finish_none(false)
                }
                Some(m) => {
                    // We have a complete match, but we've consumed at least
                    // one additional character into self.name_buf, and more
                    // in cases like
                    //
                    //     &not    => match for U+00AC
                    //     &noti   => valid prefix for &notin
                    //     &notit  => can't continue match

                    assert!(self.name_len > 0);
                    assert!(self.name_len < self.name_buf().len());
                    tokenizer.unconsume(self.name_buf().slice_from(self.name_len).to_owned());
                    let missing_semi = ';' != self.name_buf().char_at(self.name_len-1);

                    self.result = Some(CharRef {
                        chars: *m,
                        num_chars: if m[1] == '\0' { 1 } else { 2 },
                        parse_error: missing_semi,
                    });
                    Done
                }
            }
        }
    }
}
