// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate debug;

extern crate html5;

use std::io;
use std::char;
use std::default::Default;

use html5::tokenizer::{TokenSink, Token, TokenizerOpts, ParseError};
use html5::tokenizer::{CharacterTokens, NullCharacterToken, TagToken, StartTag, EndTag};
use html5::driver::{tokenize_to, one_input};

struct TokenPrinter {
    in_char_run: bool,
}

impl TokenPrinter {
    fn is_char(&mut self, is_char: bool) {
        match (self.in_char_run, is_char) {
            (false, true ) => print!("CHAR : \""),
            (true,  false) => println!("\""),
            _ => (),
        }
        self.in_char_run = is_char;
    }

    fn do_char(&mut self, c: char) {
        self.is_char(true);
        char::escape_default(c, |d| print!("{:c}", d));
    }
}

impl TokenSink for TokenPrinter {
    fn process_token(&mut self, token: Token) {
        match token {
            CharacterTokens(b) => {
                for c in b.as_slice().chars() {
                    self.do_char(c);
                }
            }
            NullCharacterToken => self.do_char('\0'),
            TagToken(tag) => {
                self.is_char(false);
                // This is not proper HTML serialization, of course.
                match tag.kind {
                    StartTag => print!("TAG  : <\x1b[32m{:s}\x1b[0m", tag.name),
                    EndTag   => print!("TAG  : <\x1b[31m/{:s}\x1b[0m", tag.name),
                }
                for attr in tag.attrs.iter() {
                    print!(" \x1b[36m{:s}\x1b[0m='\x1b[34m{:s}\x1b[0m'", attr.name, attr.value);
                }
                if tag.self_closing {
                    print!(" \x1b[31m/\x1b[0m");
                }
                println!(">");
            }
            ParseError(err) => {
                self.is_char(false);
                println!("ERROR: {:s}", err);
            }
            _ => {
                self.is_char(false);
                println!("OTHER: {}", token);
            }
        }
    }
}

fn main() {
    let mut sink = TokenPrinter {
        in_char_run: false,
    };
    let input = io::stdin().read_to_str().unwrap();
    tokenize_to(&mut sink, one_input(input), TokenizerOpts {
        profile: true,
        .. Default::default()
    });
    sink.is_char(false);
}
