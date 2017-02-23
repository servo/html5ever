#!/usr/bin/env run-cargo-script
//! This is a regular crate doc comment, but it also contains a partial
//! Cargo manifest.  Note the use of a *fenced* code block, and the
//! `cargo` "language".
//!
//! ```cargo
//! [dependencies]
//! xml5ever = "0.1.1"
//! tendril = "0.1.3"
//! ```
extern crate xml5ever;

use std::io;
use std::default::Default;

use xml5ever::tendril::{ByteTendril, ReadExt};
use xml5ever::tokenizer::{TokenSink, Token, XmlTokenizer, ParseError};
use xml5ever::tokenizer::{CharacterTokens, NullCharacterToken, TagToken};
use xml5ever::tokenizer::{PIToken, Pi, CommentToken};
use xml5ever::tokenizer::{EOFToken, DoctypeToken, Doctype};

struct SimpleTokenPrinter;


impl TokenSink for SimpleTokenPrinter {
    fn process_token(&mut self, token: Token) {
        match token {
            CharacterTokens(b) => {
                println!("TEXT: {}", &*b);
            },
            NullCharacterToken => print!("NULL"),
            TagToken(tag) => {
                println!("{:?} {} ", tag.kind, &*tag.name.local);
            },
            ParseError(err) => {
                println!("ERROR: {}", err);
            },
            PIToken(Pi{ref target, ref data}) => {
                println!("PI : <?{} {}?>", &*target, &*data);
            },
            CommentToken(ref comment) => {
                println!("<!--{:?}-->", &*comment);
            },
            EOFToken => {
                println!("EOF");
            },
            DoctypeToken(Doctype{ref name, ref public_id, ..}) => {
                println!("<!DOCTYPE {:?} {:?}>", &*name, &*public_id);
            }
        }
    }
}

fn main() {
    // Our implementation of TokenSink
    let sink = SimpleTokenPrinter;

    // We need a ByteTendril to read a file
    let mut input = ByteTendril::new();
    // Using SliceExt.read_to_tendril we can read stdin
    io::stdin().read_to_tendril(&mut input).unwrap();
    // For xml5ever we need StrTendril, so we reinterpret it
    // into StrTendril.
    let input = input.try_reinterpret().unwrap();
    // Here we create and run tokenizer
    let mut tok = XmlTokenizer::new(sink, Default::default());
    tok.feed(input);
    tok.end();
}
