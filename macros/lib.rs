// Copyright 2014 The HTML5 for Rust Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_id="html5-macros"]
#![crate_type="dylib"]

#![feature(macro_rules, plugin_registrar, quote, managed_boxes)]

extern crate syntax;
extern crate rustc;
extern crate serialize;

use rustc::plugin::Registry;

// Internal macros for use in defining other macros.

#[macro_escape]
macro_rules! bail ( ($sp:expr, $msg:expr) => ({
    cx.span_err($sp, $msg);
    return DummyResult::any($sp);
}))

#[macro_escape]
macro_rules! bail_if ( ($e:expr, $sp:expr, $msg:expr) => (
    if $e { bail!($sp, $msg) }
))

#[macro_escape]
macro_rules! expect ( ($sp:expr, $e:expr, $msg:expr) => (
    match $e {
        Some(x) => x,
        None => bail!($sp, $msg),
    }
))

mod named_entities;
mod atom;

#[macro_export]
macro_rules! unwrap_or_return ( ($opt:expr, $retval:expr) => (
    match $opt {
        None => return $retval,
        Some(x) => x,
    }
))

#[macro_export]
macro_rules! test_eq ( ($name:ident, $left:expr, $right:expr) => (
    #[test]
    fn $name() {
        assert_eq!($left, $right);
    }
))

// NB: This needs to be public or we get a linker error.
#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.register_macro("named_entities", named_entities::expand);
    reg.register_macro("static_atom_map", atom::expand_static_atom_map);
    reg.register_macro("static_atom_array", atom::expand_static_atom_array);
    reg.register_macro("atom", atom::expand_atom);
}
