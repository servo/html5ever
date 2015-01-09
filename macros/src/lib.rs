// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_name="html5ever_macros"]
#![crate_type="dylib"]

#![feature(plugin_registrar, quote, old_orphan_check)]
#![deny(warnings)]

extern crate syntax;
extern crate rustc;
extern crate serialize;

use rustc::plugin::Registry;

// Internal macros for use in defining other macros.
mod internal;

// Make these public so that rustdoc will generate documentation for them.
pub mod named_entities;
pub mod match_token;

// NB: This needs to be public or we get a linker error.
#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.register_macro("named_entities", named_entities::expand);
    reg.register_macro("match_token", match_token::expand);
}
