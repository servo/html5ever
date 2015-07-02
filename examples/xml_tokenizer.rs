#![feature(str_escape)]

extern crate xml5ever;
extern crate tendril;

use std::io::{self, Read};
use std::default::Default;
use tendril::{ByteTendril, ReadExt};

use xml5ever::tokenizer::{XTokenSink, XToken, XmlTokenizerOpts, XParseError};
use xml5ever::tokenizer::{CharacterXTokens, NullCharacterXToken, XTagToken};
use xml5ever::tokenizer::{StartXTag, EndXTag, ShortXTag, EmptyXTag};
use xml5ever::tokenizer::{PIToken, XPi};
use xml5ever::tokenize_xml_to;

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
    let mut input = ByteTendril::new();
    io::stdin().read_to_tendril(&mut input).unwrap();
    let input = input.try_reinterpret().unwrap();
    tokenize_xml_to(sink, Some(input), XmlTokenizerOpts {
        profile: true,
        exact_errors: true,
        .. Default::default()
    });
    sink.is_char(false);
}
