// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate string_cache_codegen;
extern crate phf_codegen;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{Write, BufWriter, BufReader, BufRead};
use std::path::Path;

static NAMESPACES: &'static [(&'static str, &'static str)] = &[
    ("", ""),
    ("*", "*"),
    ("html", "http://www.w3.org/1999/xhtml"),
    ("xml", "http://www.w3.org/XML/1998/namespace"),
    ("xmlns", "http://www.w3.org/2000/xmlns/"),
    ("xlink", "http://www.w3.org/1999/xlink"),
    ("svg", "http://www.w3.org/2000/svg"),
    ("mathml", "http://www.w3.org/1998/Math/MathML"),
];

fn main() {
    let generated = Path::new(&env::var("OUT_DIR").unwrap()).join("generated.rs");
    let mut generated = BufWriter::new(File::create(&generated).unwrap());

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    named_entities_to_phf(
        &Path::new(&manifest_dir).join("data").join("entities.json"),
        &Path::new(&env::var("OUT_DIR").unwrap()).join("named_entities.rs"));

    // Create a string cache for local names
    let local_names = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("local_names.txt");
    let mut local_names_atom = string_cache_codegen::AtomType::new("LocalName", "local_name!");
    for line in BufReader::new(File::open(&local_names).unwrap()).lines() {
        let local_name = line.unwrap();
        local_names_atom.atom(&local_name);
        local_names_atom.atom(&local_name.to_ascii_lowercase());
    }
    local_names_atom
        .with_macro_doc("Takes a local name as a string and returns its key in the string cache.")
        .write_to(&mut generated).unwrap();

    // Create a string cache for namespace prefixes
    string_cache_codegen::AtomType::new("Prefix", "namespace_prefix!")
        .with_macro_doc("Takes a namespace prefix string and returns its key in a string cache.")
        .atoms(NAMESPACES.iter().map(|&(prefix, _url)| prefix))
        .write_to(&mut generated)
        .unwrap();

    // Create a string cache for namespace urls
    string_cache_codegen::AtomType::new("Namespace", "namespace_url!")
        .with_macro_doc("Takes a namespace url string and returns its key in a string cache.")
        .atoms(NAMESPACES.iter().map(|&(_prefix, url)| url))
        .write_to(&mut generated)
        .unwrap();

    writeln!(generated, r#"
        /// Maps the input of `namespace_prefix!` to the output of `namespace_url!`.
        #[macro_export] macro_rules! ns {{
        "#).unwrap();
    for &(prefix, url) in NAMESPACES {
        writeln!(generated, "({}) => {{ namespace_url!({:?}) }};", prefix, url).unwrap();
    }
    writeln!(generated, "}}").unwrap();
}

fn named_entities_to_phf(from: &Path, to: &Path) {
    // A struct matching the entries in entities.json.
    #[derive(Deserialize, Debug)]
    struct CharRef {
        codepoints: Vec<u32>,
        //characters: String,  // Present in the file but we don't need it
    }

    let entities: HashMap<String, CharRef>
        = serde_json::from_reader(&mut File::open(from).unwrap()).unwrap();
    let mut entities: HashMap<&str, (u32, u32)> = entities.iter().map(|(name, char_ref)| {
        assert!(name.starts_with("&"));
        assert!(char_ref.codepoints.len() <= 2);
        (&name[1..], (char_ref.codepoints[0], *char_ref.codepoints.get(1).unwrap_or(&0)))
    }).collect();

    // Add every missing prefix of those keys, mapping to NULL characters.
    for key in entities.keys().cloned().collect::<Vec<_>>() {
        for n in 1 .. key.len() {
            entities.entry(&key[..n]).or_insert((0, 0));
        }
    }
    entities.insert("", (0, 0));

    let mut phf_map = phf_codegen::Map::new();
    for (key, value) in entities {
        phf_map.entry(key, &format!("{:?}", value));
    }

    let mut file = File::create(to).unwrap();
    writeln!(&mut file, r#"
/// A map of entity names to their codepoints. The second codepoint will
/// be 0 if the entity contains a single codepoint. Entities have their preceeding '&' removed.
///
/// # Examples
///
/// ```
/// use markup5ever::data::NAMED_ENTITIES;
///
/// assert_eq!(NAMED_ENTITIES.get("gt;").unwrap(), &(62, 0));
/// ```
"#).unwrap();
    write!(&mut file, "pub static NAMED_ENTITIES: Map<&'static str, (u32, u32)> = ").unwrap();
    phf_map.build(&mut file).unwrap();
    write!(&mut file, ";\n").unwrap();
}
