// Copyright 2015 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use match_token;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::rc::Rc;
use syntax::{ast, codemap, ext, parse, print};
use syntax::parse::token;
use syntax::parse::attr::ParserAttr;

pub fn pre_expand() {
    let mut source = String::new();
    let path = Path::new(file!()).parent().unwrap().join("../../src/tree_builder/rules.rs");
    let mut file = File::open(&path).unwrap();
    file.read_to_string(&mut source).unwrap();

    let sess = parse::ParseSess::new();
    let mut cx = ext::base::ExtCtxt::new(&sess, vec![],
        ext::expand::ExpansionConfig::default("".to_owned()));

    let tts = parse::parse_tts_from_source_str("".to_owned(), source, vec![], &sess);
    let tts = find_and_expand_match_token(&mut cx, tts);
    let tts = pretty(&mut cx, tts);

    let expanded = print::pprust::tts_to_string(&tts);
    let mut file = File::create(&path.with_extension("expanded.rs")).unwrap();
    file.write_all(expanded.as_bytes()).unwrap();
}

fn find_and_expand_match_token(cx: &mut ext::base::ExtCtxt, tts: Vec<ast::TokenTree>)
                               -> Vec<ast::TokenTree> {
    let mut expanded = Vec::new();
    let mut tts = tts.into_iter().peekable();
    while let Some(tt) = tts.next() {
        match tt {
            ast::TokenTree::TtToken(span, token::Token::Ident(ident, token::IdentStyle::Plain))
            if ident.as_str() == "match_token"
            => {
                // `!`
                if !matches!(tts.next(), Some(ast::TokenTree::TtToken(_, token::Token::Not))) {
                    expanded.push(tt);
                    continue
                }
                match tts.next() {
                    Some(ast::TokenTree::TtDelimited(_, block)) => {
                        cx.bt_push(expn_info(span));
                        expanded.extend(
                            match match_token::expand_to_tokens(cx, span, &block.tts) {
                                Ok(tts) => tts,
                                Err((span, message)) => {
                                    cx.parse_sess.span_diagnostic.span_err(span, message);
                                    panic!("Error in match_token! expansion.");
                                }
                            });
                        cx.bt_pop();
                    }
                    _ => panic!("expected a block after {:?}", span)
                }
            }
            ast::TokenTree::TtDelimited(span, mut block) => {
                block.make_unique();
                let block = Rc::try_unwrap(block).unwrap();
                expanded.push(ast::TokenTree::TtDelimited(span, Rc::new(ast::Delimited {
                    delim: block.delim,
                    open_span: block.open_span,
                    tts: find_and_expand_match_token(cx, block.tts),
                    close_span: block.close_span,
                })))
            }
            _ => expanded.push(tt)
        }
    }
    expanded
}

fn expn_info(span: codemap::Span) -> codemap::ExpnInfo {
    codemap::ExpnInfo {
        call_site: span,
        callee: codemap::NameAndSpan {
            name: "match_token".to_string(),
            format: codemap::ExpnFormat::MacroBang,
            allow_internal_unstable: false,
            span: None,
        }
    }
}

/// Somehow, going through a parser and back to tokens gives nicer whitespace.
fn pretty(cx: &mut ext::base::ExtCtxt, tts: Vec<ast::TokenTree>) -> Vec<ast::TokenTree> {
    let mut parser = parse::new_parser_from_tts(cx.parse_sess(), cx.cfg(), tts);
    let start_span = parser.span;
    let attrs = parser.parse_inner_attributes();
    let mut items = Vec::new();
    while let Some(item) = parser.parse_item() {
        items.push(item)
    }
    cx.bt_push(expn_info(start_span));
    quote_tokens!(&mut *cx, $attrs $items)
}
