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

#![feature(macro_rules, phase, globs)]
#![allow(unnecessary_parens)]

// Don't implicitly pull in things from std::*
// This helps us make a C-friendly library.
#![no_std]

extern crate alloc;

#[phase(plugin, link)]
extern crate core;

#[cfg(not(for_c))]
#[phase(plugin, link)]
extern crate std;

#[phase(plugin, link)]
extern crate collections;

#[cfg(not(for_c))]
#[phase(plugin, link)]
extern crate log;

#[phase(plugin, link)]
extern crate debug;

#[phase(plugin)]
extern crate phf_mac;

#[phase(plugin)]
extern crate macros = "html5ever-macros";

// Need #[start] for the test runner.
#[cfg(test)]
extern crate native;

extern crate phf;
extern crate time;

pub use util::atom::Atom;
pub use util::namespace::Namespace;

pub use driver::{one_input, ParseOpts, parse_to, parse};

#[cfg(not(for_c))]
pub use serialize::serialize;

mod macros;

mod util {
    #![macro_escape]

    pub mod str;
    pub mod atom;
    pub mod namespace;
    pub mod smallcharset;
}

pub mod tokenizer;
pub mod tree_builder;

#[cfg(not(for_c))]
pub mod serialize;

/// Consumers of the parser API.
#[cfg(not(for_c))]
pub mod sink {
    pub mod common;
    pub mod rcdom;
    pub mod owned_dom;
}

pub mod driver;

/// A fake `std` module so that `deriving` and other macros will work.
/// See rust-lang/rust#16803.
#[cfg(for_c)]
mod std {
    pub use core::{clone, cmp, default, fmt, option, str};
    pub use collections::hash;
}
