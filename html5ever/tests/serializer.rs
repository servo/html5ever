// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[macro_use] extern crate html5ever;

use std::default::Default;

use html5ever::{parse_fragment, parse_document, serialize, QualName};
use html5ever::driver::ParseOpts;
use html5ever::rcdom::RcDom;
use html5ever::tendril::{StrTendril, SliceExt, TendrilSink};
use html5ever::tokenizer::{Token, TokenSink, TokenSinkResult, TagKind, Tokenizer};
use html5ever::serialize::{Serialize, Serializer, TraversalScope, SerializeOpts};

use std::io;

struct Tokens(Vec<Token>);

impl TokenSink for Tokens {
    type Handle = ();

    fn process_token(&mut self, token: Token, _: u64) -> TokenSinkResult<()> {
        self.0.push(token);
        TokenSinkResult::Continue
    }
}

impl Serialize for Tokens {
    fn serialize<S>(&self, serializer: &mut S, _: TraversalScope) -> io::Result<()>
    where
        S: Serializer,
    {
        for t in self.0.iter() {
            match t {                // TODO: check whether this is an IE conditional comment or a spec comment
                &Token::TagToken(ref tag) => {
                    let name = QualName::new(
                        None,
                        "http://www.w3.org/1999/xhtml".into(),
                        tag.name.as_ref().into(),
                    );
                    match tag.kind {
                        TagKind::StartTag => {
                            serializer.start_elem(
                                name,
                                tag.attrs.iter().map(
                                    |at| (&at.name, &at.value[..]),
                                ),
                            )?
                        }
                        TagKind::EndTag => serializer.end_elem(name)?,
                    }
                }
                &Token::DoctypeToken(ref dt) => {
                    match dt.name {
                        Some(ref name) => serializer.write_doctype(&name)?,
                        None => {}
                    }
                }
                &Token::CommentToken(ref chars) => serializer.write_comment(&chars)?,
                &Token::CharacterTokens(ref chars) => serializer.write_text(&chars)?,
                &Token::NullCharacterToken |
                &Token::EOFToken => {}
                &Token::ParseError(ref e) => println!("parse error: {:#?}", e),
            }
        }
        Ok(())
    }
}

fn tokenize_and_serialize(input: StrTendril) -> StrTendril {
    let mut input = {
        let mut q = ::html5ever::tokenizer::BufferQueue::new();
        q.push_front(input.into());
        q
    };
    let mut tokenizer = Tokenizer::new(Tokens(vec![]), Default::default());
    tokenizer.feed(&mut input);
    tokenizer.end();
    let mut output = ::std::io::Cursor::new(vec![]);
    serialize(
        &mut output,
        &tokenizer.sink,
        SerializeOpts {
            create_missing_parent: true,
            ..Default::default()
        },
    ).unwrap();
    StrTendril::try_from_byte_slice(&output.into_inner()).unwrap()
}

fn parse_and_serialize(input: StrTendril) -> StrTendril {
    let dom = parse_fragment(
        RcDom::default(), ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("body")), vec![],
    ).one(input);
    let inner = &dom.document.children.borrow()[0];

    let mut result = vec![];
    serialize(&mut result, inner, Default::default()).unwrap();
    StrTendril::try_from_byte_slice(&result).unwrap()
}

macro_rules! test_fn {
    ($f:ident, $name:ident, $input:expr, $output:expr) => {
        #[test]
        fn $name() {
            assert_eq!($output, &*$f($input.to_tendril()));
        }
    };

    // Shorthand for $output = $input
    ($f:ident, $name:ident, $input:expr) => {
        test_fn!($f, $name, $input, $input);
    };
}

macro_rules! test {
    ($($t:tt)*) => {
        test_fn!(parse_and_serialize, $($t)*);
    };
}

macro_rules! test_no_parse {
    ($($t:tt)*) => {
        test_fn!(tokenize_and_serialize, $($t)*);
    };
}



test!(empty, r#""#);
test!(fuzz, "<a a=\r\n", "");
test!(smoke_test, r#"<p><i>Hello</i>, World!</p>"#);

test!(misnest, r#"<p><i>Hello!</p>, World!</i>"#,
    r#"<p><i>Hello!</i></p><i>, World!</i>"#);

test!(attr_literal, r#"<base foo="<'>">"#);
test!(attr_escape_amp, r#"<base foo="&amp;">"#);
test!(attr_escape_amp_2, r#"<base foo=&amp>"#, r#"<base foo="&amp;">"#);
test!(attr_escape_nbsp, "<base foo=x\u{a0}y>", r#"<base foo="x&nbsp;y">"#);
test!(attr_escape_quot, r#"<base foo='"'>"#, r#"<base foo="&quot;">"#);
test!(attr_escape_several, r#"<span foo=3 title='test "with" &amp;quot;'>"#,
    r#"<span foo="3" title="test &quot;with&quot; &amp;quot;"></span>"#);

test!(text_literal, r#"<p>"'"</p>"#);
test!(text_escape_amp, r#"<p>&amp;</p>"#);
test!(text_escape_amp_2, r#"<p>&amp</p>"#, r#"<p>&amp;</p>"#);
test!(text_escape_nbsp, "<p>x\u{a0}y</p>", r#"<p>x&nbsp;y</p>"#);
test!(text_escape_lt, r#"<p>&lt;</p>"#);
test!(text_escape_gt, r#"<p>&gt;</p>"#);
test!(text_escape_gt2, r#"<p>></p>"#, r#"<p>&gt;</p>"#);

test!(script_literal, r#"<script>(x & 1) < 2; y > "foo" + 'bar'</script>"#);
test!(style_literal, r#"<style>(x & 1) < 2; y > "foo" + 'bar'</style>"#);
test!(xmp_literal, r#"<xmp>(x & 1) < 2; y > "foo" + 'bar'</xmp>"#);
test!(iframe_literal, r#"<iframe>(x & 1) < 2; y > "foo" + 'bar'</iframe>"#);
test!(noembed_literal, r#"<noembed>(x & 1) < 2; y > "foo" + 'bar'</noembed>"#);
test!(noframes_literal, r#"<noframes>(x & 1) < 2; y > "foo" + 'bar'</noframes>"#);

test!(pre_lf_0, "<pre>foo bar</pre>");
test!(pre_lf_1, "<pre>\nfoo bar</pre>", "<pre>foo bar</pre>");
test!(pre_lf_2, "<pre>\n\nfoo bar</pre>", "<pre>\nfoo bar</pre>");

test!(textarea_lf_0, "<textarea>foo bar</textarea>");
test!(textarea_lf_1, "<textarea>\nfoo bar</textarea>", "<textarea>foo bar</textarea>");
test!(textarea_lf_2, "<textarea>\n\nfoo bar</textarea>", "<textarea>\nfoo bar</textarea>");

test!(listing_lf_0, "<listing>foo bar</listing>");
test!(listing_lf_1, "<listing>\nfoo bar</listing>", "<listing>foo bar</listing>");
test!(listing_lf_2, "<listing>\n\nfoo bar</listing>", "<listing>\nfoo bar</listing>");

test!(comment_1, r#"<p>hi <!--world--></p>"#);
test!(comment_2, r#"<p>hi <!-- world--></p>"#);
test!(comment_3, r#"<p>hi <!--world --></p>"#);
test!(comment_4, r#"<p>hi <!-- world --></p>"#);

// FIXME: test serialization of qualified tag/attribute names that can't be
// parsed from HTML

test!(attr_ns_1, r#"<svg xmlns="bleh"></svg>"#);
test!(attr_ns_2, r#"<svg xmlns:foo="bleh"></svg>"#);
test!(attr_ns_3, r#"<svg xmlns:xlink="bleh"></svg>"#);
test!(attr_ns_4, r#"<svg xlink:href="bleh"></svg>"#);

test_no_parse!(malformed_tokens, r#"foo</div><div>"#);

#[test]
fn doctype() {
    let dom = parse_document(
        RcDom::default(), ParseOpts::default()).one("<!doctype html>");
    dom.document.children.borrow_mut().truncate(1);  // Remove <html>
    let mut result = vec![];
    serialize(&mut result, &dom.document, Default::default()).unwrap();
    assert_eq!(String::from_utf8(result).unwrap(), "<!DOCTYPE html>");
}
