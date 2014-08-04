// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_name="html5ever-macros"]
#![crate_type="dylib"]

#![feature(macro_rules, plugin_registrar, quote, managed_boxes)]

extern crate syntax;
extern crate rustc;
extern crate serialize;
extern crate debug;

use rustc::plugin::Registry;

// Internal macros for use in defining other macros.

macro_rules! bail ( ($cx:expr, $sp:expr, $msg:expr) => ({
    $cx.span_err($sp, $msg);
    return ::syntax::ext::base::DummyResult::any($sp);
}))

macro_rules! bail_if ( ($e:expr, $cx:expr, $sp:expr, $msg:expr) => (
    if $e { bail!($cx, $sp, $msg) }
))

macro_rules! expect ( ($cx:expr, $sp:expr, $e:expr, $msg:expr) => (
    match $e {
        Some(x) => x,
        None => bail!($cx, $sp, $msg),
    }
))

// Make these public so that rustdoc will generate documentation for them.
pub mod named_entities;
pub mod atom;
pub mod match_token;

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

/// Make a tuple of the addresses of some of a struct's fields.
#[macro_export]
macro_rules! addrs_of ( ($obj:expr : $($field:ident),+) => (
    ( // make a tuple
        $(
            unsafe {
                ::std::mem::transmute::<_, uint>(&$obj.$field)
            }
        ),+
    )
))

#[macro_export]
macro_rules! format_if ( ($pred:expr, $msg_static:expr, $msg_fmt:expr, $($arg:expr),*) => (
    if $pred {
        ::std::str::Owned(format!($msg_fmt, $($arg),*))
    } else {
        ::std::str::Slice($msg_static)
    }
))

// NB: This needs to be public or we get a linker error.
#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.register_macro("named_entities", named_entities::expand);
    reg.register_macro("static_atom_map", atom::expand_static_atom_map);
    reg.register_macro("static_atom_array", atom::expand_static_atom_array);
    reg.register_macro("atom", atom::expand_atom);
    reg.register_macro("match_token", match_token::expand);
}
