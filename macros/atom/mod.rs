/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use syntax::codemap::Span;
use syntax::ast::{TokenTree, TTTok};
use syntax::ext::base::{ExtCtxt, MacResult, MacExpr};
use syntax::parse::token::{get_ident, LIT_STR, IDENT};

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

fn find_atom(t: &TokenTree) -> Option<uint> {
    let s = get_ident(match *t {
        TTTok(_, IDENT(s, _)) => s,
        TTTok(_, LIT_STR(s)) => s,
        _ => return None,
    });

    // Use bsearch instead of bsearch_elem because of type mismatch
    // between &'t str and &'static str.
    data::fast_set_atoms.bsearch(|&x| x.cmp(&s.get())).or_else(||
        data::other_atoms.bsearch(|&x| x.cmp(&s.get())).map(|i| i+64))
}

// Translate `atom!(title)` or `atom!("font-weight")` into an `Atom` constant.
pub fn expand_atom(cx: &mut ExtCtxt, sp: Span, tt: &[TokenTree]) -> Box<MacResult> {
    let i = match tt {
        [ref t] => expect!(find_atom(t), "atom!() expects an ident or string literal"),
        _ => cx.span_fatal(sp, "atom!() expects one argument"),
    };

    MacExpr::new(quote_expr!(&mut *cx,
        {
            // We need to call unchecked_static_atom_from_macro, which is
            // marked experimental so that nobody else calls it.  We can't put
            // attributes on arbitrary blocks, so we define an inner function.
            #[inline(always)]
            #[allow(experimental)]
            fn __atom_macro_inner() -> ::util::atom::Atom {
                ::util::atom::Atom::unchecked_static_atom_from_macro($i)
            }
            __atom_macro_inner()
        }
    ))
}

// Translate `atomset!(title body head)` into a static `AtomSet`.
pub fn expand_atomset(cx: &mut ExtCtxt, sp: Span, tt: &[TokenTree]) -> Box<MacResult> {
    let err = "atomset!() expects a space-separated sequence of idents and/or string literals";

    let mut bitmask: u64 = 0;
    let mut others: Vec<uint> = vec!();
    for i in tt.iter().map(|t| expect!(find_atom(t), err)) {
        if i < 64 {
            bitmask |= 1 << i;
        } else {
            others.push(i);
        }
    }

    others.sort();
    let init: Vec<TokenTree> = others.move_iter().flat_map(|i|
        quote_tokens!(&mut *cx, $i,).move_iter()
    ).collect();

    MacExpr::new(quote_expr!(&mut *cx,
        ::util::atom::AtomSet {
            bitmask: $bitmask,
            others: {
                static __atomset_macro_others: &'static [uint] = &[ $init ];
                __atomset_macro_others
            }
        }
    ))
}
