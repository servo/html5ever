// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Run a single benchmark once.  For use with profiling tools.

extern crate html5ever;

use std::io;

use html5ever::tendril::*;
use html5ever::tokenizer::{BufferQueue, Token, TokenSink, TokenSinkResult, Tokenizer};

/// In our case, our sink only contains a tokens vector
struct Sink(Vec<Token>);

impl TokenSink for Sink {
    type Handle = ();

    /// Each processed token will be handled by this method
    fn process_token(&mut self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        self.0.push(token);
        TokenSinkResult::Continue
    }
}

/// In this example we implement the TokenSink trait which lets us implement how each
/// parsed token is treated. In our example we take each token and insert it into a vector.
fn main() {
    // Read HTML from standard input
    let mut chunk = ByteTendril::new();
    io::stdin().read_to_tendril(&mut chunk).unwrap();

    // Create a buffer queue for the tokenizer
    let mut input = BufferQueue::default();
    input.push_back(chunk.try_reinterpret().unwrap());

    let mut tok = Tokenizer::new(Sink(Vec::new()), Default::default());
    let _ = tok.feed(&mut input);
    assert!(input.is_empty());
    tok.end();
}
