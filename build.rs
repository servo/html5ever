// Copyright 2014-2015 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate phf_codegen;
extern crate rustc_serialize;

use rustc_serialize::json;
use rustc_serialize::json::Json;
use rustc_serialize::Decodable;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("entities.json");
    let mut json_file = File::open(&path).ok().expect("can't open JSON file");
    let js = Json::from_reader(&mut json_file).ok().expect("can't parse JSON file");
    let map = build_map(js).expect("JSON file does not match entities.json format");

    let mut phf_map = phf_codegen::Map::new();
    for (key, &value) in map.iter() {
        phf_map.entry(&**key, &format!("{:?}", value));
    }

    let path = Path::new(env!("OUT_DIR")).join("named_entities.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());
    write!(&mut file, "pub static NAMED_ENTITIES: Map<&'static str, (u32, u32)> = ").unwrap();
    phf_map.build(&mut file).unwrap();
    write!(&mut file, ";\n").unwrap();
}

// A struct matching the entries in entities.json.
// Simplifies JSON parsing because we can use Decodable.
#[derive(RustcDecodable)]
struct CharRef {
    codepoints: Vec<u32>,
    //characters: String,  // Present in the file but we don't need it
}

// Build the map from entity names (and their prefixes) to characters.
fn build_map(js: Json) -> Option<HashMap<String, (u32, u32)>> {
    let mut map = HashMap::new();
    let json_map = match js {
        Json::Object(m) => m,
        _ => return None,
    };

    // Add every named entity to the map.
    for (k,v) in json_map.into_iter() {
        let mut decoder = json::Decoder::new(v);
        let CharRef { codepoints }: CharRef
            = Decodable::decode(&mut decoder).ok().expect("bad CharRef");

        assert!((codepoints.len() >= 1) && (codepoints.len() <= 2));
        let mut codepoints = codepoints.into_iter();
        let codepoint_pair = (codepoints.next().unwrap(), codepoints.next().unwrap_or(0));
        assert!(codepoints.next().is_none());

        // Slice off the initial '&'
        assert!(k.chars().next() == Some('&'));
        map.insert(k[1..].to_string(), codepoint_pair);
    }

    // Add every missing prefix of those keys, mapping to NULL characters.
    map.insert("".to_string(), (0, 0));
    let keys: Vec<String> = map.keys().map(|k| k.to_string()).collect();
    for k in keys.into_iter() {
        for n in 1 .. k.len() {
            let pfx = k[..n].to_string();
            if !map.contains_key(&pfx) {
                map.insert(pfx, (0, 0));
            }
        }
    }

    Some(map)
}
