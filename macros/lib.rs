/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#![crate_id="html5-macros"]
#![crate_type="dylib"]

#![feature(macro_rules, macro_registrar, quote, managed_boxes)]

extern crate syntax;
extern crate serialize;
extern crate collections;

use syntax::ast::Name;
use syntax::parse::token;
use syntax::ext::base::{SyntaxExtension, BasicMacroExpander, NormalTT};

#[macro_escape]
macro_rules! expect ( ($e:expr, $err:expr) => (
    match $e {
        Some(x) => x,
        None => cx.span_fatal(sp, $err),
    }
))

mod named_entities;
mod atom;

#[macro_export]
macro_rules! unwrap_or_return( ($opt:expr, $retval:expr) => (
    match $opt {
        None => return $retval,
        Some(x) => x,
    }
))

#[macro_export]
macro_rules! test_eq( ($name:ident, $left:expr, $right:expr) => (
    #[test]
    fn $name() {
        assert_eq!($left, $right);
    }
))

macro_rules! register( ($name:expr, $expand:expr) => (
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
}
