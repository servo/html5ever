// Copyright 2014 The HTML5 for Rust Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Run a single benchmark once.  For use with profiling tools.

extern crate test;
extern crate html5;

use std::{io, os};
use std::default::Default;

use test::black_box;

use html5::tokenizer::{TokenSink, Token, Tokenizer};

struct Sink;

impl TokenSink for Sink {
    fn process_token(&mut self, token: Token) {
        // Don't use the token, but make sure we don't get
        // optimized out entirely.
        black_box(token);
    }
}

fn main() {
    let mut path = os::self_exe_path().expect("can't get exe path");
    path.push("../data/bench/");
    path.push(os::args().get(1).as_slice());

    let mut file = io::File::open(&path).ok().expect("can't open file");
    let file_input = file.read_to_str().ok().expect("can't read file").into_string();

    let mut sink = Sink;
    let mut tok = Tokenizer::new(&mut sink, Default::default());
    tok.feed(file_input);
    tok.end();
}
