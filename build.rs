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

use rustc_serialize::json::{Json, Decoder};
use rustc_serialize::Decodable;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let rules_rs = Path::new(&manifest_dir).join("src/tree_builder/rules.rs");
    expand_match_tokens(
        &rules_rs,
        // Keep the expanded file in the source directory, so that `cargo publish` ships it.
        &rules_rs.with_extension("expanded.rs"));

    named_entities_to_phf(
        &Path::new(&manifest_dir).join("data/entities.json"),
        &Path::new(&env::var("OUT_DIR").unwrap()).join("named_entities.rs"));

    println!("cargo:rerun-if-changed={}", rules_rs.display());
}

#[cfg(feature = "codegen")]
fn expand_match_tokens(from: &Path, to: &Path) {
    extern crate html5ever_macros;

    html5ever_macros::pre_expand(from, to);
}

#[cfg(not(feature = "codegen"))]
fn expand_match_tokens(from: &Path, to: &Path) {
    use std::io::stderr;
    use std::process::exit;

    if let Err(error) = check_hash(from, to) {
        writeln!(
            stderr(),
            r"
{} is missing or not up to date with {}:
{}

Run `cargo build --features codegen` to update it.

If you’re using html5ever as a dependency, this is a bad release.
Please file an issue at https://github.com/servo/html5ever/issues/new
with the output of `cargo pkgid html5ever`.
",
            to.file_name().unwrap().to_string_lossy(),
            from.file_name().unwrap().to_string_lossy(),
            error
        ).unwrap();
        exit(1);
    }
}

#[cfg(not(feature = "codegen"))]
fn check_hash(from: &Path, to: &Path) -> Result<(), String> {
    use std::hash::{Hash, Hasher, SipHasher};
    use std::io::Read;

    // Unwrap here as the source file is expected to exist.
    let mut file_from = File::open(from).unwrap();
    let mut source = String::new();
    let mut hasher = SipHasher::new();
    file_from.read_to_string(&mut source).unwrap();
    source.hash(&mut hasher);
    let source_hash = hasher.finish();

    // IO errors from here indicate we need to regenerate the expanded file.
    let mut file_to = try!(File::open(to).map_err(|e| e.to_string()));
    let mut expanded = String::new();
    try!(file_to.read_to_string(&mut expanded).map_err(|e| e.to_string()));
    let prefix = "// source SipHash: ";
    let line = try!(expanded.lines().find(|line| line.starts_with(prefix))
                    .ok_or("source hash not found".to_string()));
    let expected_hash = try!(line[prefix.len()..].parse::<u64>().map_err(|e| e.to_string()));
    if source_hash == expected_hash {
        Ok(())
    } else {
        Err("different hash".to_string())
    }
}

fn named_entities_to_phf(from: &Path, to: &Path) {
    // A struct matching the entries in entities.json.
    #[derive(RustcDecodable)]
    struct CharRef {
        codepoints: Vec<u32>,
        //characters: String,  // Present in the file but we don't need it
    }

    let json = Json::from_reader(&mut File::open(from).unwrap()).unwrap();
    let entities: HashMap<String, CharRef> = Decodable::decode(&mut Decoder::new(json)).unwrap();
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
    write!(&mut file, "pub static NAMED_ENTITIES: Map<&'static str, (u32, u32)> = ").unwrap();
    phf_map.build(&mut file).unwrap();
    write!(&mut file, ";\n").unwrap();
}
