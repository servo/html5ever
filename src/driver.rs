// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! High-level interface to the parser.

use tokenizer::{TokenizerOpts, Tokenizer, TokenSink};
use tree_builder::{TreeBuilderOpts, TreeBuilder, TreeSink};

use std::option;
use std::default::Default;

use tokenizer::{XmlTokenizerOpts, XmlTokenizer, XTokenSink};
use tree_builder::{ XmlTreeBuilder};

use collections::string::String;

use string_cache::{Atom, QualName};

/// Convenience function to turn a single `String` into an iterator.
pub fn one_input(x: String) -> option::IntoIter<String> {
    Some(x).into_iter()
}

/// Tokenize and send results to a `TokenSink`.
///
/// ## Example
///
/// ```ignore
/// let mut sink = MySink;
/// tokenize_to(&mut sink, one_input(my_str), Default::default());
/// ```
pub fn tokenize_to<
        Sink: TokenSink,
        It: Iterator<Item=String>
    >(
        sink: Sink,
        input: It,
        opts: TokenizerOpts) -> Sink {

    let mut tok = Tokenizer::new(sink, opts);
    for s in input {
        tok.feed(s);
    }
    tok.end();
    tok.unwrap()
}

/// Tokenize and send results to a `XTokenSink`.
///
/// ## Example
///
/// ```ignore
/// let mut sink = MySink;
/// tokenize_xml_to(&mut sink, one_input(my_str), Default::default());
/// ```
pub fn tokenize_xml_to<
        Sink: XTokenSink,
        It: Iterator<Item=String>
    >(
        sink: Sink,
        input: It,
        opts: XmlTokenizerOpts) -> Sink {

    let mut tok = XmlTokenizer::new(sink, opts);
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
/// let mut sink = MySink;
/// parse_to(&mut sink, one_input(my_str), Default::default());
/// ```
pub fn parse_to<
        Sink: TreeSink,
        It: Iterator<Item=String>
    >(
        sink: Sink,
        input: It,
        opts: ParseOpts) -> Sink {

    let tb = TreeBuilder::new(sink, opts.tree_builder);
    let mut tok = Tokenizer::new(tb, opts.tokenizer);
    for s in input {
        tok.feed(s);
    }
    tok.end();
    tok.unwrap().unwrap()
}

/// Parse and send results to a `TreeSink`.
///
/// ## Example
///
/// ```ignore
/// let mut sink = MySink;
/// parse_xml_to(&mut sink, one_input(my_str), Default::default());
/// ```
pub fn parse_xml_to<
        Sink:TreeSink,
        It: Iterator<Item=String>
    >(
        sink: Sink,
        input: It,
        opts: XmlTokenizerOpts) -> Sink {

    let tb = XmlTreeBuilder::new(sink);
    let mut tok = XmlTokenizer::new(tb, opts);
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
/// let mut sink = MySink;
/// parse_fragment_to(&mut sink, one_input(my_str), context_token, Default::default());
/// ```
pub fn parse_fragment_to<
        Sink: TreeSink,
        It: Iterator<Item=String>
    >(
        sink: Sink,
        input: It,
        context: Atom,
        opts: ParseOpts) -> Sink {

    let mut sink = sink;
    let context_elem = sink.create_element(QualName::new(ns!(HTML), context), vec!());
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
          It: Iterator<Item=String>,
{
    let sink = parse_to(Default::default(), input, opts);
    ParseResult::get_result(sink)
}

/// Parse into a type which implements `ParseResult`.
///
/// ## Example
///
/// ```ignore
/// let dom: RcDom = parse_xml(one_input(my_str), Default::default());
/// ```
pub fn parse_xml<Output, It>(input: It, opts: XmlTokenizerOpts) -> Output
    where Output: ParseResult,
          It: Iterator<Item=String>,
{
    let sink = parse_xml_to(Default::default(), input, opts);
    ParseResult::get_result(sink)
}


/// Parse an HTML fragment into a type which implements `ParseResult`.
///
/// ## Example
///
/// ```ignore
/// let dom: RcDom = parse_fragment(one_input(my_str), context_token, Default::default());
/// ```
pub fn parse_fragment<Output, It>(input: It, context: Atom, opts: ParseOpts) -> Output
    where Output: ParseResult,
          It: Iterator<Item=String>,
{
    let sink = parse_fragment_to(Default::default(), input, context, opts);
    ParseResult::get_result(sink)
}
