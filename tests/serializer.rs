// Copyright 2015 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![cfg_attr(feature = "unstable", feature(plugin))]
#![cfg_attr(feature = "unstable", plugin(string_cache_plugin))]

#[macro_use] extern crate string_cache;
extern crate tendril;
extern crate html5ever;

use std::default::Default;

use tendril::{StrTendril, SliceExt};

use html5ever::driver::ParseOpts;
use html5ever::{parse_fragment, parse, one_input, serialize};
use html5ever::rcdom::RcDom;

fn parse_and_serialize(input: StrTendril) -> StrTendril {
    let dom: RcDom = parse_fragment(one_input(input),
                                    qualname!(HTML, body),
                                    vec![],
                                    ParseOpts::default());
    let inner = &dom.document.borrow().children[0];

    let mut result = vec![];
    serialize(&mut result, inner, Default::default()).unwrap();
    StrTendril::try_from_byte_slice(&result).unwrap()
}

macro_rules! test {
    ($name:ident, $input:expr, $output:expr) => {
        #[test]
        fn $name() {
            assert_eq!($output, &*parse_and_serialize($input.to_tendril()));
        }
    };

    // Shorthand for $output = $input
    ($name:ident, $input:expr) => {
        test!($name, $input, $input);
    };
}

test!(empty, r#""#);
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
test!(pre_lf_2, "<pre>\n\nfoo bar</pre>");

test!(textarea_lf_0, "<textarea>foo bar</textarea>");
test!(textarea_lf_1, "<textarea>\nfoo bar</textarea>", "<textarea>foo bar</textarea>");
test!(textarea_lf_2, "<textarea>\n\nfoo bar</textarea>");

test!(listing_lf_0, "<listing>foo bar</listing>");
test!(listing_lf_1, "<listing>\nfoo bar</listing>", "<listing>foo bar</listing>");
test!(listing_lf_2, "<listing>\n\nfoo bar</listing>");

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

#[test]
fn doctype() {
    let dom: RcDom = parse(one_input("<!doctype html>".to_tendril()), ParseOpts::default());
    dom.document.borrow_mut().children.truncate(1);  // Remove <html>
    let mut result = vec![];
    serialize(&mut result, &dom.document, Default::default()).unwrap();
    assert_eq!(String::from_utf8(result).unwrap(), "<!DOCTYPE html>\n");
}
