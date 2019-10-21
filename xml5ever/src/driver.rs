// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::tokenizer::{XmlTokenizer, XmlTokenizerOpts};
use crate::tree_builder::{TreeSink, XmlTreeBuilder, XmlTreeBuilderOpts};

use std::borrow::Cow;

use markup5ever::buffer_queue::BufferQueue;
use crate::tendril;
use crate::tendril::stream::{TendrilSink, Utf8LossyDecoder};
use crate::tendril::StrTendril;

/// All-encompasing parser setting structure.
#[derive(Clone, Default)]
pub struct XmlParseOpts {
    /// Xml tokenizer options.
    pub tokenizer: XmlTokenizerOpts,
    /// Xml tree builder .
    pub tree_builder: XmlTreeBuilderOpts,
}

/// Parse and send results to a `TreeSink`.
///
/// ## Example
///
/// ```ignore
/// let mut sink = MySink;
/// parse_document(&mut sink, iter::once(my_str), Default::default());
/// ```
pub fn parse_document<Sink>(sink: Sink, opts: XmlParseOpts) -> XmlParser<Sink>
where
    Sink: TreeSink,
{
    let tb = XmlTreeBuilder::new(sink, opts.tree_builder);
    let tok = XmlTokenizer::new(tb, opts.tokenizer);
    XmlParser {
        tokenizer: tok,
        input_buffer: BufferQueue::new(),
    }
}

/// An XML parser,
/// ready to receive Unicode input through the `tendril::TendrilSink` trait’s methods.
pub struct XmlParser<Sink>
where
    Sink: TreeSink,
{
    /// Tokenizer used by XmlParser.
    pub tokenizer: XmlTokenizer<XmlTreeBuilder<Sink::Handle, Sink>>,
    /// Input used by XmlParser.
    pub input_buffer: BufferQueue,
}

impl<Sink: TreeSink> TendrilSink<tendril::fmt::UTF8> for XmlParser<Sink> {
    type Output = Sink::Output;

    fn process(&mut self, t: StrTendril) {
        self.input_buffer.push_back(t);
        self.tokenizer.feed(&mut self.input_buffer);
    }

    // FIXME: Is it too noisy to report every character decoding error?
    fn error(&mut self, desc: Cow<'static, str>) {
        self.tokenizer.sink.sink.parse_error(desc)
    }

    fn finish(mut self) -> Self::Output {
        self.tokenizer.end();
        self.tokenizer.sink.sink.finish()
    }
}

impl<Sink: TreeSink> XmlParser<Sink> {
    /// Wrap this parser into a `TendrilSink` that accepts UTF-8 bytes.
    ///
    /// Use this when your input is bytes that are known to be in the UTF-8 encoding.
    /// Decoding is lossy, like `String::from_utf8_lossy`.
    pub fn from_utf8(self) -> Utf8LossyDecoder<Self> {
        Utf8LossyDecoder::new(self)
    }
}

#[cfg(test)]
mod tests {
    extern crate markup5ever_rcdom;
    use super::*;
    use self::markup5ever_rcdom::{RcDom, SerializableHandle};
    use crate::serialize::serialize;
    use tendril::TendrilSink;

    #[test]
    fn el_ns_serialize() {
        assert_eq_serialization(
            "<a:title xmlns:a=\"http://www.foo.org/\" value=\"test\">Test</a:title>",
            parse_document(RcDom::default(), XmlParseOpts::default())
                .from_utf8()
                .one(
                    "<a:title xmlns:a=\"http://www.foo.org/\" value=\"test\">Test</title>"
                        .as_bytes(),
                ),
        );
    }

    #[test]
    fn nested_ns_serialize() {
        assert_eq_serialization("<a:x xmlns:a=\"http://www.foo.org/\" xmlns:b=\"http://www.bar.org/\" value=\"test\"><b:y/></a:x>",
            parse_document(RcDom::default(), XmlParseOpts::default())
                .from_utf8()
                .one("<a:x xmlns:a=\"http://www.foo.org/\" xmlns:b=\"http://www.bar.org/\" value=\"test\"><b:y/></a:x>".as_bytes()));
    }

    #[test]
    fn def_ns_serialize() {
        assert_eq_serialization(
            "<table xmlns=\"html4\"><td></td></table>",
            parse_document(RcDom::default(), XmlParseOpts::default())
                .from_utf8()
                .one("<table xmlns=\"html4\"><td></td></table>".as_bytes()),
        );
    }

    #[test]
    fn undefine_ns_serialize() {
        assert_eq_serialization(
            "<a:x xmlns:a=\"http://www.foo.org\"><a:y xmlns:a=\"\"><a:z/></a:y</a:x>",
            parse_document(RcDom::default(), XmlParseOpts::default())
                .from_utf8()
                .one(
                    "<a:x xmlns:a=\"http://www.foo.org\"><a:y xmlns:a=\"\"><a:z/></a:y</a:x>"
                        .as_bytes(),
                ),
        );
    }

    #[test]
    fn redefine_default_ns_serialize() {
        assert_eq_serialization(
            "<x xmlns=\"http://www.foo.org\"><y xmlns=\"\"><z/></y</x>",
            parse_document(RcDom::default(), XmlParseOpts::default())
                .from_utf8()
                .one("<x xmlns=\"http://www.foo.org\"><y xmlns=\"\"><z/></y</x>".as_bytes()),
        );
    }

    #[test]
    fn attr_serialize() {
        assert_serialization(
            "<title value=\"test\">Test</title>",
            parse_document(RcDom::default(), XmlParseOpts::default())
                .from_utf8()
                .one("<title value='test'>Test".as_bytes()),
        );
    }

    #[test]
    fn from_utf8() {
        assert_serialization(
            "<title>Test</title>",
            parse_document(RcDom::default(), XmlParseOpts::default())
                .from_utf8()
                .one("<title>Test".as_bytes()),
        );
    }

    fn assert_eq_serialization(text: &'static str, dom: RcDom) {
        let mut serialized = Vec::new();
        let document: SerializableHandle = dom.document.clone().into();
        serialize(&mut serialized, &document, Default::default()).unwrap();

        let dom_from_text = parse_document(RcDom::default(), XmlParseOpts::default())
            .from_utf8()
            .one(text.as_bytes());

        let mut reserialized = Vec::new();
        let document: SerializableHandle = dom_from_text.document.clone().into();
        serialize(
            &mut reserialized,
            &document,
            Default::default(),
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(serialized).unwrap(),
            String::from_utf8(reserialized).unwrap()
        );
    }

    fn assert_serialization(text: &'static str, dom: RcDom) {
        let mut serialized = Vec::new();
        let document: SerializableHandle = dom.document.clone().into();
        serialize(&mut serialized, &document, Default::default()).unwrap();
        assert_eq!(String::from_utf8(serialized).unwrap(), text);
    }
}
