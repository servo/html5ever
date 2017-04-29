// Copyright 2016 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![cfg_attr(feature = "heap_size", feature(proc_macro))]
#[cfg(feature = "heap_size")] #[macro_use] extern crate heapsize_derive;
#[cfg(feature = "heap_size")] extern crate heapsize;
extern crate string_cache;
extern crate phf;
extern crate tendril;

#[macro_export]
macro_rules! qualname {
    ("", $local:tt) => {
        ::markup5ever::QualName {
            ns: ns!(),
            prefix: None,
            local: local_name!($local),
        }
    };
    ($ns:tt, $local:tt) => {
        ::markup5ever::QualName {
            ns: ns!($ns),
            prefix: None,
            local: local_name!($local),
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/generated.rs"));


pub mod data;
pub mod interface;
}

pub use interface::{QualName, Attribute};
