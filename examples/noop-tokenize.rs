// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Run a single benchmark once.  For use with profiling tools.

#![feature(core, env, os, io, test, path)]

extern crate test;
extern crate html5ever;

use std::old_io as io;
use std::env;
use std::default::Default;

use test::black_box;

use html5ever::tokenizer::{TokenSink, Token, TokenizerOpts};
use html5ever::driver::{tokenize_to, one_input};

struct Sink;

impl TokenSink for Sink {
    fn process_token(&mut self, token: Token) {
        // Don't use the token, but make sure we don't get
        // optimized out entirely.
        black_box(token);
    }
}

fn main() {
    let mut path = env::current_exe().ok().expect("can't get exe path");
    path.push("../data/bench/");
    path.push(env::args().nth(1).unwrap().into_string().unwrap().as_slice());

    let mut file = io::File::open(&path).ok().expect("can't open file");
    let file_input = file.read_to_string().ok().expect("can't read file");

    tokenize_to(Sink, one_input(file_input), TokenizerOpts {
        profile: true,
        .. Default::default()
    });
}
