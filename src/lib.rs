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

#![cfg_attr(test, deny(warnings))]
#![allow(unused_parens)]

#![cfg_attr(feature = "heap_size", feature(plugin, custom_derive))]
#![cfg_attr(feature = "heap_size", plugin(heapsize_plugin))]
#[cfg(feature = "heap_size")]
extern crate heapsize;

#[cfg(feature = "hyper")] extern crate hyper;

#[macro_use]
extern crate log;

#[macro_use]
extern crate string_cache;

#[macro_use]
extern crate mac;

extern crate phf;

extern crate time;

pub use tokenizer::Attribute;
pub use driver::{ParseOpts, parse_document, parse_fragment, Parser};

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
pub mod rcdom;

/// Re-export the tendril crate.
pub mod tendril {
    extern crate tendril;
    pub use self::tendril::*;
}

/// Re-export the encoding crate.
pub use tendril::encoding;
