// Copyright 2016 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate string_cache_codegen;

use std::ascii::AsciiExt;
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

    let local_names = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("local_names.txt");
    let mut local_names_atom = string_cache_codegen::AtomType::new("LocalName", "local_name!");
    for line in BufReader::new(File::open(&local_names).unwrap()).lines() {
        let local_name = line.unwrap();
        local_names_atom.atom(&local_name);
        local_names_atom.atom(&local_name.to_ascii_lowercase());
    }
    local_names_atom.write_to(&mut generated).unwrap();

    string_cache_codegen::AtomType::new("Prefix", "namespace_prefix!")
        .atoms(NAMESPACES.iter().map(|&(prefix, _url)| prefix))
        .write_to(&mut generated)
        .unwrap();

    string_cache_codegen::AtomType::new("Namespace", "namespace_url!")
        .atoms(NAMESPACES.iter().map(|&(_prefix, url)| url))
        .write_to(&mut generated)
        .unwrap();

    writeln!(generated, "#[macro_export] macro_rules! ns {{").unwrap();
    for &(prefix, url) in NAMESPACES {
        writeln!(generated, "({}) => {{ namespace_url!({:?}) }};", prefix, url).unwrap();
    }
    writeln!(generated, "}}").unwrap();
}
