// Copyright 2015 The xml5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use tokenizer::{XmlTokenizerOpts, XmlTokenizer, TokenSink};
use tree_builder::{TreeSink, XmlTreeBuilder};

use tendril;
use tendril::{StrTendril};
/// Parse and send results to a `TreeSink`.
///
/// ## Example
///
/// ```ignore
/// let mut sink = MySink;
/// parse_to(&mut sink, iter::once(my_str), Default::default());
/// ```
pub fn parse_to<
        Sink:TreeSink,
        It: IntoIterator<Item=tendril::StrTendril>
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


/// Parse into a type which implements `ParseResult`.
///
/// ## Example
///
/// ```ignore
/// let dom: RcDom = parse(iter::once(my_str), Default::default());
/// ```
pub fn parse<Output, It>(input: It, opts: XmlTokenizerOpts) -> Output
    where Output: ParseResult,
          It: IntoIterator<Item=tendril::StrTendril>,
{
    let sink = parse_to(Default::default(), input, opts);
    ParseResult::get_result(sink)
}

/// Results which can be extracted from a `TreeSink`.
///
/// Implement this for your parse tree data type so that it
/// can be returned by `parse()`.
pub trait ParseResult {
    /// Type of consumer of tree modifications.
    /// It also extends `Default` for convenience.
    type Sink: TreeSink + Default;
    /// Returns parsed tree data type
    fn get_result(sink: Self::Sink) -> Self;
}
#[cfg(test)]
mod tests {
    use rcdom::RcDom;
    use serialize::serialize;
    use std::iter::repeat;
    use tendril::TendrilSink;
    use super::*;

    #[test]
    fn from_utf8() {
        assert_serialization(
            parse_document(RcDom::default(), XmlParseOpts::default())
                .from_utf8()
                .one("<title>Test".as_bytes()));
    }

    #[test]
    fn from_bytes_one() {
        assert_serialization(
            parse_document(RcDom::default(), XmlParseOpts::default())
                .from_bytes(BytesOpts::default())
                .one("<title>Test".as_bytes()));
    }

    #[test]
    fn from_bytes_iter() {
        assert_serialization(
            parse_document(RcDom::default(), XmlParseOpts::default())
                .from_bytes(BytesOpts::default())
                .from_iter([
                    "<title>Test".as_bytes(),
                    repeat(' ').take(1200).collect::<String>().as_bytes(),
                ].iter().cloned()));
    }

    fn assert_serialization(dom: RcDom) {
        let mut serialized = Vec::new();
        serialize(&mut serialized, &dom.document, Default::default()).unwrap();
        assert_eq!(String::from_utf8(serialized).unwrap().replace(" ", ""),
                   "<title>Test</title>");
    }
}
