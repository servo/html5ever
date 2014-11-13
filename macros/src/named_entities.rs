// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unused_imports)]  // for quotes

use std::io;
use std::path;
use serialize::json;
use serialize::json::Json;
use serialize::Decodable;
use std::collections::HashMap;

use syntax::codemap::Span;
use syntax::ast::{Path, ExprLit, Lit_, TokenTree, TtToken};
use syntax::parse::token;
use syntax::ext::base::{ExtCtxt, MacResult, MacExpr};
use syntax::ext::source_util::expand_file;

// A struct matching the entries in entities.json.
// Simplifies JSON parsing because we can use Decodable.
#[deriving(Decodable)]
struct CharRef {
    // codepoints: Vec<u32>, // Present in the file but we don't need it
    characters: String,
}

// Build the map from entity names (and their prefixes) to characters.
fn build_map(js: Json) -> Option<HashMap<String, (String, u8)>> {
    let mut map = HashMap::new();
    let json_map = match js {
        json::Object(m) => m,
        _ => return None,
    };

    // Add every named entity to the map.
    for (k,v) in json_map.into_iter() {
        let mut decoder = json::Decoder::new(v);
        let CharRef { characters }: CharRef
            = Decodable::decode(&mut decoder).ok().expect("bad CharRef");

        let num_chars = characters.as_slice().chars().count();

        // Slice off the initial '&'
        assert!(k.as_slice().char_at(0) == '&');
        map.insert(k.as_slice().slice_from(1).to_string(), (characters, num_chars.to_u8().unwrap()));
    }

    // Add every missing prefix of those keys, mapping to NULL characters.
    map.insert("".to_string(), ("".to_string(), 0));
    let keys: Vec<String> =
        map.keys()
           .map(|k| k.to_string())
           .collect();
    for k in keys.into_iter() {
        for n in range(1, k.len()) {
            let pfx = k.as_slice().slice_to(n).to_string();

            if !map.contains_key(&pfx) {
                map.insert(pfx, ("".to_string(), 0));
            }
        }
    }

    Some(map)
}

// Expand named_entities!("path/to/entities.json") into an invocation of phf_map!().
pub fn expand(cx: &mut ExtCtxt, sp: Span, tt: &[TokenTree]) -> Box<MacResult+'static> {
    let usage = "Usage: named_entities!(\"path/to/entities.json\")";

    // Argument to the macro should be a single literal string: a path to
    // entities.json, relative to the file containing the macro invocation.
    let json_filename = match tt {
        [TtToken(_, token::Literal(token::Lit::Str_(s), _))] => s.as_str().to_string(),
        _ => bail!(cx, sp, usage),
    };

    // Get the result of calling file!() in the same place as our macro.
    // This would be a lot nicer if @-patterns were still supported.
    let mod_filename = expect!(cx, sp, match expand_file(cx, sp, &[]).make_expr() {
        Some(e) => match e.node {
            ExprLit(ref s) => match s.node {
                Lit_::LitStr(ref s, _) => Some(s.get().to_string()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }, "unexpected result from file!()");

    // Combine those to get an absolute path to entities.json.
    let mod_path: path::Path = expect!(cx, sp, from_str(mod_filename.as_slice()),
        "can't parse module filename");
    let json_path = mod_path.dir_path().join(json_filename);

    // Open the JSON file, parse it, and build the map from names to characters.
    let mut json_file = expect!(cx, sp, io::File::open(&json_path).ok(),
        "can't open JSON file");
    let js = expect!(cx, sp, json::from_reader(&mut json_file as &mut Reader).ok(),
        "can't parse JSON file");
    let map = expect!(cx, sp, build_map(js),
        "JSON file does not match entities.json format");

    // Emit a macro invocation of the form
    //
    //     phf_map!(k => v, k => v, ...)
    let mut toks: Vec<TokenTree> = vec!();
    for (k, (v, len)) in map.into_iter() {
        let k = k.as_slice();
        let v = v.as_slice();
        toks.extend(quote_tokens!(&mut *cx, $k => ($v, $len),).into_iter());
    }
    MacExpr::new(quote_expr!(&mut *cx, phf_map!($toks)))
}
