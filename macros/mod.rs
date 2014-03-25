/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[crate_id="html5-macros"];
#[crate_type="dylib"];

#[feature(macro_rules, macro_registrar, quote, managed_boxes)];

extern crate extra;
extern crate syntax;
extern crate serialize;
extern crate collections;

use syntax::ast::Name;
use syntax::parse::token;
use syntax::ext::base::{SyntaxExtension, BasicMacroExpander, NormalTT};

mod named_entities;

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

// NB: This needs to be public or we get a linker error.
#[macro_registrar]
pub fn macro_registrar(register: |Name, SyntaxExtension|) {
    register(token::intern(&"named_entities"),
        NormalTT(~BasicMacroExpander {
            expander: named_entities::expand,
            span: None
        },
        None));
}
