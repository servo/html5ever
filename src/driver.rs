// Copyright 2014 The HTML5 for Rust Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use tokenizer::{TokenizerOpts, Tokenizer, TokenSink};
use tree_builder::{TreeBuilderOpts, TreeBuilder, TreeSink};

use std::default::Default;
use std::option;

pub fn one_input(x: String) -> option::Item<String> {
    Some(x).move_iter()
}

pub fn tokenize_to<
        Sink: TokenSink,
        It: Iterator<String>
    >(
        sink: &mut Sink,
        mut input: It,
        opts: TokenizerOpts) {

    let mut tok = Tokenizer::new(sink, opts);
    for s in input {
        tok.feed(s);
    }
    tok.end();
}

#[deriving(Clone, Default)]
pub struct ParseOpts {
    pub tokenizer: TokenizerOpts,
    pub tree_builder: TreeBuilderOpts,
}

pub fn parse_to<
        Handle: Clone,
        Sink: TreeSink<Handle>,
        It: Iterator<String>
    >(
        sink: &mut Sink,
        mut input: It,
        opts: ParseOpts) {

    let mut tb  = TreeBuilder::new(sink, opts.tree_builder);
    let mut tok = Tokenizer::new(&mut tb, opts.tokenizer);
    for s in input {
        tok.feed(s);
    }
    tok.end();
}

pub trait ParseResult<Sink> {
    fn get_result(sink: Sink) -> Self;
}

pub fn parse<
        Handle: Clone,
        Sink: Default + TreeSink<Handle>,
        Output: ParseResult<Sink>,
        It: Iterator<String>
    >(
        input: It,
        opts: ParseOpts) -> Output {

    let mut sink: Sink = Default::default();
    parse_to(&mut sink, input, opts);
    ParseResult::get_result(sink)
}
