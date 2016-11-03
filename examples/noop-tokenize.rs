// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Run a single benchmark once.  For use with profiling tools.

extern crate html5ever;
extern crate tendril;

use std::io;
use std::default::Default;

use tendril::{ByteTendril, ReadExt};

use html5ever::tokenizer::{ProcessResult, TokenSink, Token, Tokenizer};
use html5ever::tokenizer::buffer_queue::BufferQueue;

struct Sink(Vec<Token>);

impl TokenSink for Sink {
    fn process_token(&mut self, token: Token) -> ProcessResult {
        // Don't use the token, but make sure we don't get
        // optimized out entirely.
        self.0.push(token);
        ProcessResult::Continue
    }
}

fn main() {
    let mut chunk = ByteTendril::new();
    io::stdin().read_to_tendril(&mut chunk).unwrap();
    let mut input = BufferQueue::new();
    input.push_back(chunk.try_reinterpret().unwrap());

    let mut tok = Tokenizer::new(Sink(Vec::new()), Default::default());
    tok.feed(&mut input);
    assert!(input.is_empty());
    tok.end();
}
