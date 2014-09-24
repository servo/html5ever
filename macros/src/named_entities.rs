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
use std::collections::hashmap::HashMap;

use syntax::codemap::Span;
use syntax::ast::{Path, ExprLit, LitStr, TokenTree, TTTok};
use syntax::parse::token::LIT_STR;
use syntax::ext::base::{ExtCtxt, MacResult, MacExpr};
use syntax::ext::source_util::expand_file;

// A struct matching the entries in entities.json.
// Simplifies JSON parsing because we can use Decodable.
#[deriving(Decodable)]
struct CharRef {
    codepoints: Vec<u32>,
    //characters: String,  // Present in the file but we don't need it
}

// Build the map from entity names (and their prefixes) to characters.
fn build_map(js: Json) -> Option<HashMap<String, [u32, ..2]>> {
    let mut map = HashMap::new();
    let json_map = match js {
        json::Object(m) => m,
        _ => return None,
    };

    // Add every named entity to the map.
    for (k,v) in json_map.into_iter() {
        let mut decoder = json::Decoder::new(v);
        let CharRef { codepoints }: CharRef
            = Decodable::decode(&mut decoder).ok().expect("bad CharRef");

        assert!((codepoints.len() >= 1) && (codepoints.len() <= 2));
        let mut codepoint_pair = [0, 0];
        for (i,n) in codepoints.into_iter().enumerate() {
            codepoint_pair[i] = n;
        }

        // Slice off the initial '&'
        assert!(k.as_slice().char_at(0) == '&');
        map.insert(k.as_slice().slice_from(1).to_string(), codepoint_pair);
    }

    // Add every missing prefix of those keys, mapping to NULL characters.
    map.insert("".to_string(), [0, 0]);
    let keys: Vec<String> = map.keys().map(|k| k.to_string()).collect();
    for k in keys.into_iter() {
        for n in range(1, k.len()) {
            let pfx = k.as_slice().slice_to(n).to_string();
            if !map.contains_key(&pfx) {
                map.insert(pfx, [0, 0]);
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
        [TTTok(_, LIT_STR(s))] => s.as_str().to_string(),
        _ => bail!(cx, sp, usage),
    };

    // Get the result of calling file!() in the same place as our macro.
    // This would be a lot nicer if @-patterns were still supported.
    let mod_filename = expect!(cx, sp, match expand_file(cx, sp, &[]).make_expr() {
        Some(e) => match e.node {
            ExprLit(ref s) => match s.node {
                LitStr(ref s, _) => Some(s.get().to_string()),
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
    let mut tts: Vec<TokenTree> = vec!();
    for (k, c) in map.into_iter() {
        let k = k.as_slice();
        let c0 = c[0];
        let c1 = c[1];
        tts.extend(quote_tokens!(&mut *cx, $k => [$c0, $c1],).into_iter());
    }
    MacExpr::new(quote_expr!(&mut *cx, phf_map!($tts)))
}
