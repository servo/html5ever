// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! High-level interface to the parser.

use tokenizer::{Attribute, TokenSink, Tokenizer, TokenizerOpts};
use tree_builder::{TreeBuilderOpts, TreeBuilder, TreeSink};

use std::option;
use std::default::Default;

use string_cache::QualName;
use tendril::StrTendril;

/// Convenience function to turn a single value into an iterator.
pub fn one_input<T>(x: T) -> option::IntoIter<T> {
    Some(x).into_iter()
}

/// Tokenize and send results to a `TokenSink`.
///
/// ## Example
///
/// ```ignore
/// let sink = MySink;
/// tokenize_to(&mut sink, one_input(my_str), Default::default());
/// ```
pub fn tokenize_to<Sink, It>(sink: Sink, input: It, opts: TokenizerOpts) -> Sink
    where Sink: TokenSink,
          It: Iterator<Item=StrTendril>,
{
    let mut tok = Tokenizer::new(sink, opts);
    for s in input {
        tok.feed(s);
    }
    tok.end();
    tok.unwrap()
}

/// All-encompassing options struct for the parser.
#[derive(Clone, Default)]
pub struct ParseOpts {
    /// Tokenizer options.
    pub tokenizer: TokenizerOpts,

    /// Tree builder options.
    pub tree_builder: TreeBuilderOpts,
}

/// Parse and send results to a `TreeSink`.
///
/// ## Example
///
/// ```ignore
/// let sink = MySink;
/// parse_to(sink, one_input(my_str), Default::default());
/// ```
pub fn parse_to<Sink, It>(sink: Sink, input: It, opts: ParseOpts) -> Sink
    where Sink: TreeSink,
          It: Iterator<Item=StrTendril>,
{
    let tb = TreeBuilder::new(sink, opts.tree_builder);
    let mut tok = Tokenizer::new(tb, opts.tokenizer);
    for s in input {
        tok.feed(s);
    }
    tok.end();
    tok.unwrap().unwrap()
}

/// Parse an HTML fragment and send results to a `TreeSink`.
///
/// ## Example
///
/// ```ignore
/// let sink = MySink;
/// parse_fragment_to(sink, one_input(my_str), context_name, context_attrs, Default::default());
/// ```
pub fn parse_fragment_to<Sink, It>(mut sink: Sink,
                                   input: It,
                                   context_name: QualName,
                                   context_attrs: Vec<Attribute>,
                                   opts: ParseOpts) -> Sink
    where Sink: TreeSink,
          It: Iterator<Item=StrTendril>
{
    let context_elem = sink.create_element(context_name, context_attrs);
    let tb = TreeBuilder::new_for_fragment(sink, context_elem, None, opts.tree_builder);
    let tok_opts = TokenizerOpts {
        initial_state: Some(tb.tokenizer_state_for_context_elem()),
        .. opts.tokenizer
    };
    let mut tok = Tokenizer::new(tb, tok_opts);
    for s in input {
        tok.feed(s);
    }
    tok.end();
    tok.unwrap().unwrap()
}

/// Results which can be extracted from a `TreeSink`.
///
/// Implement this for your parse tree data type so that it
/// can be returned by `parse()`.
pub trait ParseResult {
    type Sink: TreeSink + Default;
    fn get_result(sink: Self::Sink) -> Self;
}

/// Parse into a type which implements `ParseResult`.
///
/// ## Example
///
/// ```ignore
/// let dom: RcDom = parse(one_input(my_str), Default::default());
/// ```
pub fn parse<Output, It>(input: It, opts: ParseOpts) -> Output
    where Output: ParseResult,
          It: Iterator<Item=StrTendril>,
{
    let sink = parse_to(Default::default(), input, opts);
    ParseResult::get_result(sink)
}

/// Parse an HTML fragment into a type which implements `ParseResult`.
///
/// ## Example
///
/// ```ignore
/// let dom: RcDom = parse_fragment(
///     one_input(my_str), context_name, context_attrs, Default::default());
/// ```
pub fn parse_fragment<Output, It>(input: It,
                                  context_name: QualName,
                                  context_attrs: Vec<Attribute>,
                                  opts: ParseOpts) -> Output
    where Output: ParseResult,
          It: Iterator<Item=StrTendril>,
{
    let sink = parse_fragment_to(Default::default(), input, context_name, context_attrs, opts);
    ParseResult::get_result(sink)
}
