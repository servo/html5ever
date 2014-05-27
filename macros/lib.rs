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

#![feature(macro_rules, macro_registrar, quote, managed_boxes)]

extern crate syntax;
extern crate serialize;
extern crate collections;

use syntax::ast::Name;
use syntax::parse::token;
use syntax::ext::base::{SyntaxExtension, BasicMacroExpander, NormalTT};

// Internal macros for use in defining other macros.

#[macro_escape]
macro_rules! bail ( ($msg:expr) => ({
    cx.span_err(sp, $msg);
    return DummyResult::any(sp);
}))

#[macro_escape]
macro_rules! bail_if ( ($e:expr, $msg:expr) => (
    if $e { bail!($msg) }
))

#[macro_escape]
macro_rules! expect ( ($e:expr, $msg:expr) => (
    match $e {
        Some(x) => x,
        None => bail!($msg),
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

// Wrap the procedural macro match_atom_impl! so that the
// scrutinee expression is always a single token tree.
#[macro_export]
macro_rules! match_atom ( ($scrutinee:expr $body:tt) => (
    match_atom_impl!(($scrutinee) $body)
))

macro_rules! register ( ($name:expr, $expand:expr) => (
    register(token::intern($name),
        NormalTT(box BasicMacroExpander {
            expander: $expand,
            span: None
        },
        None));
))

// NB: This needs to be public or we get a linker error.
#[macro_registrar]
pub fn macro_registrar(register: |Name, SyntaxExtension|) {
    register!("named_entities", named_entities::expand);
    register!("static_atom_map", atom::expand_static_atom_map);
    register!("static_atom_array", atom::expand_static_atom_array);
    register!("atom", atom::expand_atom);
    register!("match_atom_impl", atom::expand_match_atom_impl);
}
