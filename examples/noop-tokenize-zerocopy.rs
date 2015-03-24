// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(test)]

extern crate test;
extern crate html5ever;

use std::io;
use std::default::Default;

use test::black_box;

use html5ever::TendrilReader;
use html5ever::tokenizer::{TokenSink, Token, TokenizerOpts};
use html5ever::driver::tokenize_to;

struct Sink;

impl TokenSink for Sink {
    fn process_token(&mut self, token: Token) {
        // Don't use the token, but make sure we don't get
        // optimized out entirely.
        black_box(token);
    }
}

fn main() {
    let reader = TendrilReader::from_utf8(16384, io::stdin())
        .map(|r| r.unwrap());

    tokenize_to(Sink, reader, TokenizerOpts {
        profile: true,
        .. Default::default()
    });
}
