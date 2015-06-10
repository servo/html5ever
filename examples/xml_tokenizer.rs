// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(collections)]

extern crate html5ever;

use std::io::{self, Read};
use std::default::Default;

use html5ever::tokenizer::{XTokenSink, XToken, XmlTokenizerOpts, XParseError};
use html5ever::tokenizer::{CharacterXTokens, NullCharacterXToken, XTagToken};
use html5ever::tokenizer::{StartXTag, EndXTag, ShortXTag, EmptyXTag};
use html5ever::tokenizer::{PIToken, XPi};
use html5ever::driver::{tokenize_xml_to, one_input};

#[derive(Copy, Clone)]
struct XTokenPrinter {
    in_char_run: bool,
}

impl XTokenPrinter {
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
        print!("{}", c.to_string().escape_default());
    }
}

impl XTokenSink for XTokenPrinter {
    fn process_token(&mut self, token: XToken) {
        match token {
            CharacterXTokens(b) => {
                for c in b.chars() {
                    self.do_char(c);
                }
            }
            NullCharacterXToken => self.do_char('\0'),
            XTagToken(tag) => {
                self.is_char(false);
                // This is not proper HTML serialization, of course.
                match tag.kind {
                    StartXTag => print!("TAG  : <\x1b[32m{}\x1b[0m", tag.name.as_slice()),
                    EndXTag   => print!("END TAG  : <\x1b[31m/{}\x1b[0m", tag.name.as_slice()),
                    ShortXTag => print!("Short TAG  : <\x1b[31m/{}\x1b[0m", tag.name.as_slice()),
                    EmptyXTag => print!("Empty TAG  : <\x1b[31m{}\x1b[0m", tag.name.as_slice()),
                }
                for attr in tag.attrs.iter() {
                    print!(" \x1b[36m{}\x1b[0m='\x1b[34m{}\x1b[0m'",
                        attr.name.local.as_slice(), attr.value);
                }
                if tag.kind == EmptyXTag {
                    print!("/");
                }
                println!(">");
            }
            XParseError(err) => {
                self.is_char(false);
                println!("ERROR: {}", err);
            }
            PIToken(XPi{target, data}) => {
                self.is_char(false);
                println!("PI : <?{:?} {:?}?>", target, data);
            }
            _ => {
                self.is_char(false);
                println!("OTHER: {:?}", token);
            }
        }
    }
}

fn main() {
    let mut sink = XTokenPrinter {
        in_char_run: false,
    };
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    tokenize_xml_to(sink, one_input(input), XmlTokenizerOpts {
        profile: true,
        exact_errors: true,
        .. Default::default()
    });
    sink.is_char(false);
}
