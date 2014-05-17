/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use syntax::codemap::Span;
use syntax::ast::TokenTree;
use syntax::ext::base::{ExtCtxt, MacResult, MacExpr};

use std::iter::Chain;
use std::slice::Items;

mod data;

fn all_atoms<'a>() -> Chain<Items<'a, &'static str>, Items<'a, &'static str>> {
    data::fast_set_atoms.iter().chain(data::other_atoms.iter())
}

// Build a PhfMap yielding static atom IDs.
// Takes no arguments.
pub fn expand_static_atom_map(cx: &mut ExtCtxt, sp: Span, tt: &[TokenTree]) -> Box<MacResult> {
    if tt.len() > 0 {
        cx.span_fatal(sp, "static_atom_map!() expects no arguments");
    }

    let tts: Vec<TokenTree> = all_atoms().enumerate().flat_map(|(i, k)|
        quote_tokens!(&mut *cx, $k => $i,).move_iter()
    ).collect();
    MacExpr::new(quote_expr!(&mut *cx, phf_map!($tts)))
}

// Build the array to convert IDs back to strings.
// FIXME: share storage with the PhfMap keys.
pub fn expand_static_atom_array(cx: &mut ExtCtxt, sp: Span, tt: &[TokenTree]) -> Box<MacResult> {
    if tt.len() > 0 {
        cx.span_fatal(sp, "static_atom_array!() expects no arguments");
    }

    let tts: Vec<TokenTree> = all_atoms().flat_map(|k|
        quote_tokens!(&mut *cx, $k,).move_iter()
    ).collect();
    MacExpr::new(quote_expr!(&mut *cx, &[$tts]))
}
