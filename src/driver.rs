// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! High-level interface to the parser.

use tokenizer::{Attribute, Tokenizer, TokenizerOpts};
use tree_builder::{TreeBuilderOpts, TreeBuilder, TreeSink};

use std::borrow::Cow;
use std::mem;

use encoding::{self, EncodingRef};
use string_cache::QualName;
use tendril;
use tendril::{StrTendril, ByteTendril};
use tendril::stream::{TendrilSink, Utf8LossyDecoder, LossyDecoder};

/// All-encompassing options struct for the parser.
#[derive(Clone, Default)]
pub struct ParseOpts {
    /// Tokenizer options.
    pub tokenizer: TokenizerOpts,

    /// Tree builder options.
    pub tree_builder: TreeBuilderOpts,
}

/// Parse an HTML document
///
/// The returned value implements `tendril::TendrilSink`
/// so that Unicode input may be provided incrementally,
/// or all at once with the `one` method.
///
/// If your input is bytes, use `Parser::from_utf8` or `Parser::from_bytes`.
pub fn parse_document<Sink>(sink: Sink, opts: ParseOpts) -> Parser<Sink> where Sink: TreeSink {
    let tb = TreeBuilder::new(sink, opts.tree_builder);
    let tok = Tokenizer::new(tb, opts.tokenizer);
    Parser { tokenizer: tok }
}

/// Parse an HTML fragment
///
/// The returned value implements `tendril::TendrilSink`
/// so that Unicode input may be provided incrementally,
/// or all at once with the `one` method.
///
/// If your input is bytes, use `Parser::from_utf8` or `Parser::from_bytes`.
pub fn parse_fragment<Sink>(mut sink: Sink, opts: ParseOpts,
                            context_name: QualName, context_attrs: Vec<Attribute>)
                            -> Parser<Sink>
                            where Sink: TreeSink {
    let context_elem = sink.create_element(context_name, context_attrs);
    parse_fragment_for_element(sink, opts, context_elem, None)
}

/// Like `parse_fragment`, but with an existing context element
/// and optionally a form element.
pub fn parse_fragment_for_element<Sink>(sink: Sink, opts: ParseOpts,
                                        context_element: Sink::Handle,
                                        form_element: Option<Sink::Handle>)
                                        -> Parser<Sink>
                                        where Sink: TreeSink {
    let tb = TreeBuilder::new_for_fragment(sink, context_element, form_element, opts.tree_builder);
    let tok_opts = TokenizerOpts {
        initial_state: Some(tb.tokenizer_state_for_context_elem()),
        .. opts.tokenizer
    };
    let tok = Tokenizer::new(tb, tok_opts);
    Parser { tokenizer: tok }
}

/// An HTML parser,
/// ready to recieve Unicode input through the `tendril::TendrilSink` trait’s methods.
pub struct Parser<Sink> where Sink: TreeSink {
    pub tokenizer: Tokenizer<TreeBuilder<Sink::Handle, Sink>>,
}

impl<Sink: TreeSink> TendrilSink<tendril::fmt::UTF8> for Parser<Sink> {
    fn process(&mut self, t: StrTendril) {
        self.tokenizer.feed(t)
    }

    // FIXME: Is it too noisy to report every character decoding error?
    fn error(&mut self, desc: Cow<'static, str>) {
        self.tokenizer.sink_mut().sink_mut().parse_error(desc)
    }

    type Output = Sink::Output;

    fn finish(mut self) -> Self::Output {
        self.tokenizer.end();
        self.tokenizer.unwrap().unwrap().finish()
    }
}

impl<Sink: TreeSink> Parser<Sink> {
    /// Wrap this parser into a `TendrilSink` that accepts UTF-8 bytes.
    ///
    /// Use this when your input is bytes that are known to be in the UTF-8 encoding.
    /// Decoding is lossy, like `String::from_utf8_lossy`.
    pub fn from_utf8(self) -> Utf8LossyDecoder<Self> {
        Utf8LossyDecoder::new(self)
    }

    /// Wrap this parser into a `TendrilSink` that accepts bytes
    /// and tries to detect the correct character encoding.
    ///
    /// Currently this looks for a Byte Order Mark,
    /// then uses `BytesOpts::transport_layer_encoding`,
    /// then falls back to UTF-8.
    ///
    /// FIXME(https://github.com/servo/html5ever/issues/18): this should look for `<meta>` elements
    /// and other data per
    /// https://html.spec.whatwg.org/multipage/syntax.html#determining-the-character-encoding
    pub fn from_bytes(self, opts: BytesOpts) -> BytesParser<Sink> {
        BytesParser {
            state: BytesParserState::Initial { parser: self },
            opts: opts,
        }
    }
}

/// Options for choosing a character encoding
#[derive(Clone, Default)]
pub struct BytesOpts {
    /// The character encoding specified by the transport layer, if any.
    /// In HTTP for example, this is the `charset` parameter of the `Content-Type` response header.
    pub transport_layer_encoding: Option<EncodingRef>,
}

/// An HTML parser,
/// ready to recieve bytes input through the `tendril::TendrilSink` trait’s methods.
///
/// See `Parser::from_bytes`.
pub struct BytesParser<Sink> where Sink: TreeSink {
    state: BytesParserState<Sink>,
    opts: BytesOpts,
}

enum BytesParserState<Sink> where Sink: TreeSink {
    Initial {
        parser: Parser<Sink>,
    },
    Buffering {
        parser: Parser<Sink>,
        buffer: ByteTendril
    },
    Parsing {
        decoder: LossyDecoder<Parser<Sink>>,
    },
    Transient
}

impl<Sink: TreeSink> BytesParser<Sink> {
    /// Access the underlying Parser
    pub fn str_parser(&self) -> &Parser<Sink> {
        match self.state {
            BytesParserState::Initial { ref parser } => parser,
            BytesParserState::Buffering { ref parser, .. } => parser,
            BytesParserState::Parsing { ref decoder } => decoder.inner_sink(),
            BytesParserState::Transient => unreachable!(),
        }
    }

    /// Access the underlying Parser
    pub fn str_parser_mut(&mut self) -> &mut Parser<Sink> {
        match self.state {
            BytesParserState::Initial { ref mut parser } => parser,
            BytesParserState::Buffering { ref mut parser, .. } => parser,
            BytesParserState::Parsing { ref mut decoder } => decoder.inner_sink_mut(),
            BytesParserState::Transient => unreachable!(),
        }
    }

    /// Insert a Unicode chunk in the middle of the byte stream.
    ///
    /// This is e.g. for supporting `document.write`.
    pub fn process_unicode(&mut self, t: StrTendril) {
        if t.is_empty() {
            return  // Don’t prevent buffering/encoding detection
        }
        if let BytesParserState::Parsing { ref mut decoder } = self.state {
            decoder.inner_sink_mut().process(t)
        } else {
            match mem::replace(&mut self.state, BytesParserState::Transient) {
                BytesParserState::Initial { mut parser } => {
                    parser.process(t);
                    self.start_parsing(parser, ByteTendril::new())
                }
                BytesParserState::Buffering { parser, buffer } => {
                    self.start_parsing(parser, buffer);
                    if let BytesParserState::Parsing { ref mut decoder } = self.state {
                        decoder.inner_sink_mut().process(t)
                    } else {
                        unreachable!()
                    }
                }
                BytesParserState::Parsing { .. } | BytesParserState::Transient => unreachable!(),
            }
        }
    }

    fn start_parsing(&mut self, parser: Parser<Sink>, buffer: ByteTendril) {
        let encoding = detect_encoding(&buffer, &self.opts);
        let mut decoder = LossyDecoder::new(encoding, parser);
        decoder.process(buffer);
        self.state = BytesParserState::Parsing { decoder: decoder }
    }
}

impl<Sink: TreeSink> TendrilSink<tendril::fmt::Bytes> for BytesParser<Sink> {
    fn process(&mut self, t: ByteTendril) {
        if let &mut BytesParserState::Parsing { ref mut decoder } = &mut self.state {
            return decoder.process(t)
        }
        let (parser, buffer) = match mem::replace(&mut self.state, BytesParserState::Transient) {
            BytesParserState::Initial{ parser } => (parser, t),
            BytesParserState::Buffering { parser, mut buffer } => {
                buffer.push_tendril(&t);
                (parser, buffer)
            }
            BytesParserState::Parsing { .. } | BytesParserState::Transient => unreachable!(),
        };
        if buffer.len32() >= PRESCAN_BYTES {
            self.start_parsing(parser, buffer)
        } else {
            self.state = BytesParserState::Buffering {
                parser: parser,
                buffer: buffer,
            }
        }
    }

    fn error(&mut self, desc: Cow<'static, str>) {
        match self.state {
            BytesParserState::Initial { ref mut parser } => parser.error(desc),
            BytesParserState::Buffering { ref mut parser, .. } => parser.error(desc),
            BytesParserState::Parsing { ref mut decoder } => decoder.error(desc),
            BytesParserState::Transient => unreachable!(),
        }
    }

    type Output = Sink::Output;

    fn finish(self) -> Self::Output {
        match self.state {
            BytesParserState::Initial { parser } => parser.finish(),
            BytesParserState::Buffering { parser, buffer } => {
                let encoding = detect_encoding(&buffer, &self.opts);
                let mut decoder = LossyDecoder::new(encoding, parser);
                decoder.process(buffer);
                decoder.finish()
            },
            BytesParserState::Parsing { decoder } => decoder.finish(),
            BytesParserState::Transient => unreachable!(),
        }
    }
}

/// How many bytes does detect_encoding() need
// FIXME(#18): should be 1024 for <meta> elements.
const PRESCAN_BYTES: u32 = 3;

/// https://html.spec.whatwg.org/multipage/syntax.html#determining-the-character-encoding
fn detect_encoding(bytes: &ByteTendril, opts: &BytesOpts) -> EncodingRef {
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        return encoding::all::UTF_8
    }
    if bytes.starts_with(b"\xFE\xFF") {
        return encoding::all::UTF_16BE
    }
    if bytes.starts_with(b"\xFF\xFE") {
        return encoding::all::UTF_16LE
    }
    if let Some(encoding) = opts.transport_layer_encoding {
        return encoding
    }
    // FIXME(#18): <meta> etc.
    return encoding::all::UTF_8
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
            parse_document(RcDom::default(), ParseOpts::default())
                .from_utf8()
                .one("<title>Test".as_bytes()));
    }

    #[test]
    fn from_bytes_one() {
        assert_serialization(
            parse_document(RcDom::default(), ParseOpts::default())
                .from_bytes(BytesOpts::default())
                .one("<title>Test".as_bytes()));
    }

    #[test]
    fn from_bytes_iter() {
        assert_serialization(
            parse_document(RcDom::default(), ParseOpts::default())
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
                   "<html><head><title>Test</title></head><body></body></html>");
    }
}
