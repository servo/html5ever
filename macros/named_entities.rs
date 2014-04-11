/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::io;
use std::path;
use serialize::json;
use serialize::json::Json;
use serialize::Decodable;
use collections::hashmap::HashMap;

use syntax::codemap::Span;
use syntax::ast::{Path, ExprLit, LitStr, TokenTree, TTTok};
use syntax::parse::token::{get_ident, LIT_STR};
use syntax::ext::base::{ExtCtxt, MacResult, MRExpr};
use syntax::ext::source_util::expand_file;

macro_rules! expect ( ($e:expr, $err:expr) => (
    match $e {
        Some(x) => x,
        None => cx.span_fatal(sp, $err),
    }
))

// A struct matching the entries in entities.json.
// Simplifies JSON parsing because we can use Decodable.
#[deriving(Decodable)]
struct CharRef {
    codepoints: ~[u32],
    //characters: ~str,  // Present in the file but we don't need it
}

// Build the map from entity names (and their prefixes) to characters.
fn build_map(js: Json) -> Option<HashMap<~str, [u32, ..2]>> {
    let mut map = HashMap::new();
    let json_map = match js {
        json::Object(m) => m,
        _ => return None,
    };

    // Add every named entity to the map.
    for (k,v) in json_map.move_iter() {
        let mut decoder = json::Decoder::new(v);
        let CharRef { codepoints }: CharRef = Decodable::decode(&mut decoder);

        assert!((codepoints.len() >= 1) && (codepoints.len() <= 2));
        let mut codepoint_pair = [0, 0];
        for (i,n) in codepoints.move_iter().enumerate() {
            codepoint_pair[i] = n;
        }

        // Slice off the initial '&'
        assert!(k.char_at(0) == '&');
        map.insert(k.slice_from(1).to_owned(), codepoint_pair);
    }

    // Add every missing prefix of those keys, mapping to NULL characters.
    map.insert(~"", [0, 0]);
    let keys: ~[~str] = map.keys().map(|k| k.to_owned()).collect();
    for k in keys.move_iter() {
        for n in range(1, k.len()) {
            let pfx = k.slice_to(n).to_owned();
            if !map.contains_key(&pfx) {
                map.insert(pfx, [0, 0]);
            }
        }
    }

    Some(map)
}

// Expand named_entities!("path/to/entities.json") into an invocation of phf_map!().
pub fn expand(cx: &mut ExtCtxt, sp: Span, tt: &[TokenTree]) -> MacResult {
    // Argument to the macro should be a single literal string: a path to
    // entities.json, relative to the file containing the macro invocation.
    let json_filename = match tt {
        [TTTok(_, LIT_STR(s))] => get_ident(s).get().to_owned(),
        _ => cx.span_fatal(sp, "named_entities takes one argument, a relative path to entities.json"),
    };

    // Get the result of calling file!() in the same place as our macro.
    // This would be a lot nicer if @-patterns were still supported.
    let mod_filename = expect!(match expand_file(cx, sp, &[]) {
        MRExpr(e) => match e.node {
            ExprLit(s) => match s.node {
                LitStr(ref s, _) => Some(s.get().to_owned()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }, "unexpected result from file!()");

    // Combine those to get an absolute path to entities.json.
    let mod_path: path::Path = expect!(from_str(mod_filename), "can't parse module filename");
    let json_path = mod_path.dir_path().join(json_filename);

    // Open the JSON file, parse it, and build the map from names to characters.
    let mut json_file = expect!(io::File::open(&json_path).ok(), "can't open JSON file");
    let js = expect!(json::from_reader(&mut json_file as &mut Reader).ok(), "can't parse JSON file");
    let map = expect!(build_map(js), "JSON file does not match entities.json format");

    // Emit a macro invocation of the form
    //
    //     phf_map!(k => v, k => v, ...)
    let mut tts: Vec<TokenTree> = Vec::new();
    for (k, [c1, c2]) in map.move_iter() {
        tts.push_all_move(quote_tokens!(&mut *cx, $k => [$c1, $c2],));
    }
    MRExpr(quote_expr!(&mut *cx, phf_map!($tts)))
}
