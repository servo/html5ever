/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

// Run a single benchmark once.  For use with profiling tools.

extern crate test;
extern crate html5;

use std::{io, os};
use std::default::Default;
use std::strbuf::StrBuf;

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
    path.push(os::args()[1]);

    let mut file = io::File::open(&path).ok().expect("can't open file");
    let file_input = StrBuf::from_owned_str(file.read_to_str().ok().expect("can't read file"));

    let mut sink = Sink;
    let mut tok = Tokenizer::new(&mut sink, Default::default());
    tok.feed(file_input);
    tok.end();
}
