// Copyright 2014-2017  The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[macro_use] extern crate quote;
#[macro_use] extern crate syn;
extern crate proc_macro2;

use std::env;
use std::path::Path;

#[path = "macros/match_token.rs"]
mod match_token;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let rules_rs = Path::new(&manifest_dir).join("src/tree_builder/rules.rs");
    match_token::expand(
        &rules_rs,
        &Path::new(&env::var("OUT_DIR").unwrap()).join("rules.rs"));

    println!("cargo:rerun-if-changed={}", rules_rs.display());
}
