// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_name="html5ever"]
#![crate_type="dylib"]

#![feature(plugin, box_syntax, core, collections, str_char, slice_patterns)]
#![deny(warnings)]
#![allow(unused_parens)]

#![plugin(phf_macros)]
#![plugin(string_cache_plugin)]
#![plugin(html5ever_macros)]

#[macro_use]
extern crate log;

#[macro_use]
extern crate string_cache;

#[macro_use]
extern crate mac;

extern crate phf;

extern crate time;

pub use tokenizer::Attribute;
pub use driver::{one_input, ParseOpts, parse_to, parse_fragment_to, parse, parse_fragment};
pub use driver::{parse_xml, parse_xml_to, tokenize_xml_to};

pub use serialize::serialize;

#[macro_use]
mod macros;

#[macro_use]
mod util {
    pub mod str;
    #[macro_use] pub mod smallcharset;
}

pub mod tokenizer;
pub mod tree_builder;

pub mod serialize;

pub mod driver;
