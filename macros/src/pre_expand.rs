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
use std::hash::{Hash, Hasher, SipHasher};
use std::io::{Read, Write};
use std::path::Path;
use std::rc::Rc;
use syntax::{ast, codemap, ext, parse, print};
use syntax::parse::token;

pub fn pre_expand(from: &Path, to: &Path) {
    let mut source = String::new();
    let mut file_from = File::open(from).unwrap();
    file_from.read_to_string(&mut source).unwrap();

    let mut file_to = File::create(to).unwrap();
    write_header(&from, &source, &mut file_to);

    let sess = parse::ParseSess::new();
    let mut feature_gated_cfgs = Vec::new();
    let mut cx = ext::base::ExtCtxt::new(&sess, vec![],
        ext::expand::ExpansionConfig::default("".to_owned()),
        &mut feature_gated_cfgs);

    let from = from.to_string_lossy().into_owned();
    let tts = parse::parse_tts_from_source_str(from, source, vec![], &sess);
    let tts = find_and_expand_match_token(&mut cx, tts);
    let tts = pretty(&mut cx, tts);

    let expanded = print::pprust::tts_to_string(&tts);
    file_to.write_all(expanded.as_bytes()).unwrap();
}

fn find_and_expand_match_token(cx: &mut ext::base::ExtCtxt, tts: Vec<ast::TokenTree>)
                               -> Vec<ast::TokenTree> {
    let mut expanded = Vec::new();
    let mut tts = tts.into_iter().peekable();
    while let Some(tt) = tts.next() {
        match tt {
            ast::TokenTree::Token(span, token::Token::Ident(ident, token::IdentStyle::Plain))
            if ident.name.as_str() == "match_token"
            => {
                // `!`
                if !matches!(tts.next(), Some(ast::TokenTree::Token(_, token::Token::Not))) {
                    expanded.push(tt);
                    continue
                }
                match tts.next() {
                    Some(ast::TokenTree::Delimited(_, block)) => {
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
            ast::TokenTree::Delimited(span, mut block) => {
                Rc::make_mut(&mut block);
                let block = Rc::try_unwrap(block).unwrap();
                expanded.push(ast::TokenTree::Delimited(span, Rc::new(ast::Delimited {
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
            format: codemap::ExpnFormat::MacroBang(token::intern("match_token")),
            allow_internal_unstable: false,
            span: None,
        }
    }
}

/// Somehow, going through a parser and back to tokens gives nicer whitespace.
fn pretty(cx: &mut ext::base::ExtCtxt, tts: Vec<ast::TokenTree>) -> Vec<ast::TokenTree> {
    let mut parser = parse::new_parser_from_tts(cx.parse_sess(), cx.cfg(), tts);
    let start_span = parser.span;
    let mut items = Vec::new();
    let attrs = parser.parse_inner_attributes().unwrap();
    while let Ok(Some(item)) = parser.parse_item() {
        items.push(item)
    }
    cx.bt_push(expn_info(start_span));
    quote_tokens!(&mut *cx, $attrs $items)
}

fn write_header(source_file_name: &Path, source: &str, file: &mut File) {
    let mut hasher = SipHasher::new();
    source.hash(&mut hasher);
    let source_hash = hasher.finish();

    for header_line in source.lines().take_while(|line| line.starts_with("//")) {
        writeln!(file, "{}", header_line).unwrap();
    }
    writeln!(file, r"
// This file is generated from {}
// source SipHash: {}
",
    source_file_name.file_name().unwrap().to_string_lossy(), source_hash).unwrap();
}
