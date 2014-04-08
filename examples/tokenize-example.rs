/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate html5;

use std::io;
use std::char;

use html5::tokenizer::{TokenSink, Token, Tokenizer, TokenizerOpts, ParseError};
use html5::tokenizer::{CharacterToken, MultiCharacterToken, TagToken, StartTag, EndTag};

struct TokenPrinter {
    in_char_run: bool,
}

impl TokenPrinter {
    fn is_char(&mut self, is_char: bool) {
        match (self.in_char_run, is_char) {
            (false, true ) => print!("CHAR : "),
            (true,  false) => println!(""),
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
            CharacterToken(c) => {
                self.do_char(c);
            }
            MultiCharacterToken(b) => {
                for c in b.chars() {
                    self.do_char(c);
                }
            }
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
                println!("OTHER: {:?}", token);
            }
        }
    }
}

fn main() {
    let mut sink = TokenPrinter {
        in_char_run: false,
    };
    {
        let mut tok = Tokenizer::new(&mut sink, TokenizerOpts {
            profile: true,
            .. Default::default()
        });
        tok.feed(io::stdin().read_to_str().unwrap());
        tok.end();
    }
    sink.is_char(false);
}
