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

use string_cache::QualName;
use tendril;
use tendril::StrTendril;
use tendril::stream::{TendrilSink, Utf8LossyDecoder};

/// All-encompassing options struct for the parser.
#[derive(Clone, Default)]
pub struct ParseOpts {
    /// Tokenizer options.
    pub tokenizer: TokenizerOpts,

    /// Tree builder options.
    pub tree_builder: TreeBuilderOpts,
}

/// Parse an HTML document
pub fn parse_document<Sink>(sink: Sink, opts: ParseOpts) -> Parser<Sink> where Sink: TreeSink {
    let tb = TreeBuilder::new(sink, opts.tree_builder);
    let tok = Tokenizer::new(tb, opts.tokenizer);
    Parser { tokenizer: tok }
}

/// Parse an HTML fragment
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

pub struct Parser<Sink> where Sink: TreeSink {
    tokenizer: Tokenizer<TreeBuilder<Sink::Handle, Sink>>
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
    pub fn from_utf8(self) -> Utf8LossyDecoder<Self> {
        Utf8LossyDecoder::new(self)
    }
}
