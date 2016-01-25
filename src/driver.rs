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
#[cfg(feature = "hyper")] use hyper::client::IntoUrl;
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
    let tb = TreeBuilder::new_for_fragment(sink, context_elem, None, opts.tree_builder);
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
    tokenizer: Tokenizer<TreeBuilder<Sink::Handle, Sink>>,
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

    /// Fetch an HTTP or HTTPS URL with Hyper and parse.
    #[cfg(feature = "hyper")]
    pub fn from_http<U: IntoUrl>(self, url: U) -> Result<Sink::Output, ::hyper::Error> {
        use hyper::Client;
        use hyper::header::ContentType;
        use hyper::mime::Attr::Charset;
        use encoding::label::encoding_from_whatwg_label;

        let mut response = try!(Client::new().get(url).send());
        let opts = BytesOpts {
            transport_layer_encoding: response.headers.get::<ContentType>()
                .and_then(|content_type| content_type.get_param(Charset))
                .and_then(|charset| encoding_from_whatwg_label(charset))
        };
        Ok(try!(self.from_bytes(opts).read_from(&mut response)))
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
            let encoding = detect_encoding(&buffer, &self.opts);
            let decoder = LossyDecoder::new(encoding, parser);
            self.state = BytesParserState::Parsing { decoder: decoder }
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
                let decoder = LossyDecoder::new(encoding, parser);
                decoder.finish()
            },
            BytesParserState::Parsing { decoder } => decoder.finish(),
            BytesParserState::Transient => unreachable!(),
        }
    }
}

/// How many bytes does detect_encoding() need
// NOTE: 3 would be enough for a BOM, but 1024 is specified for <meta> elements.
const PRESCAN_BYTES: u32 = 1024;

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
    // FIXME: <meta> etc.
    return encoding::all::UTF_8
}
