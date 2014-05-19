/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use syntax::codemap::Span;
use syntax::ast::{TokenTree, TTTok, TTDelim};
use syntax::ext::base::{ExtCtxt, MacResult, MacExpr, DummyResult};
use syntax::parse::token::{get_ident, LIT_STR, IDENT};
use syntax::parse::token::{LBRACE, RBRACE, FAT_ARROW, COMMA, UNDERSCORE};

use std::iter::Chain;
use std::slice::Items;
use std::rc::Rc;

mod data;

fn all_atoms<'a>() -> Chain<Items<'a, &'static str>, Items<'a, &'static str>> {
    data::fast_set_atoms.iter().chain(data::other_atoms.iter())
}

// Build a PhfMap yielding static atom IDs.
// Takes no arguments.
pub fn expand_static_atom_map(cx: &mut ExtCtxt, sp: Span, tt: &[TokenTree]) -> Box<MacResult> {
    bail_if!(tt.len() != 0, "Usage: static_atom_map!()");
    let tts: Vec<TokenTree> = all_atoms().enumerate().flat_map(|(i, k)|
        quote_tokens!(&mut *cx, $k => $i,).move_iter()
    ).collect();
    MacExpr::new(quote_expr!(&mut *cx, phf_map!($tts)))
}

// Build the array to convert IDs back to strings.
// FIXME: share storage with the PhfMap keys.
pub fn expand_static_atom_array(cx: &mut ExtCtxt, sp: Span, tt: &[TokenTree]) -> Box<MacResult> {
    bail_if!(tt.len() != 0, "Usage: static_atom_array!()");
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
    let usage = "Usage: atom!(html) or atom!(\"font-weight\")";
    let i = match tt {
        [ref t] => expect!(find_atom(t), usage),
        _ => bail!(usage),
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

fn get_brace_block(tt: &TokenTree) -> Option<Rc<Vec<TokenTree>>> {
    match tt {
        &TTDelim(ref body) if body.len() >= 2 => {
            match (body.get(0), body.get(body.len()-1)) {
                (&TTTok(_, LBRACE), &TTTok(_, RBRACE)) => Some(body.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

// Expand `match_atom_impl!()`, used by `match_atom!()`.
pub fn expand_match_atom_impl(cx: &mut ExtCtxt, sp: Span, tt: &[TokenTree]) -> Box<MacResult> {
    // FIXME: Ugly parsing code.  Might be nicer to use syntax::parse::parser.

    let usage = "Usage: match_atom!(e { html head => ..., _ => ... })";
    bail_if!(tt.len() < 2, usage);

    // Can't splice individual token trees, only vectors.
    let scrutinee = vec!(tt[0].clone());

    // Get an iterator for the match body, except for the first and last tokens
    // (the curly braces themselves).
    let body = expect!(get_brace_block(&tt[1]), usage);
    let mut body = body.as_slice().slice(1, body.len()-1).iter();

    let mut expanded: Vec<TokenTree> = vec!();
    'outer: loop {
        // Collect atoms to the left of =>.
        let mut arm = vec!();
        loop {
            match body.next() {
                None => break 'outer,
                Some(&TTTok(_, FAT_ARROW)) => break,
                Some(t @ &TTTok(_, UNDERSCORE)) => {
                    arm.push(t.clone());
                }
                Some(t) => {
                    let id = expect!(find_atom(t), "can't parse atom");
                    if !arm.is_empty() {
                        arm.push_all_move(quote_tokens!(&mut *cx, |));
                    }
                    arm.push_all_move(quote_tokens!(&mut *cx, Some($id)));
                }
            }
        }

        // Collect either a brace-delimeted block or everything up to a comma.
        let block = match body.next() {
            None => bail!("RHS missing"),
            Some(t @ &TTDelim(_)) => expect!(get_brace_block(t), "block not brace-delimited"),
            Some(t) => {
                let mut v = vec!(t.clone());
                loop {
                    match body.next() {
                        None | Some(&TTTok(_, COMMA)) => break,
                        Some(t) => v.push(t.clone()),
                    }
                }
                Rc::new(v)
            }
        };

        expanded.push_all_move(quote_tokens!(&mut *cx,
            $arm => { $block }
        ));
    }

    MacExpr::new(quote_expr!(&mut *cx,
        match $scrutinee.get_static_atom_id_from_macro() {
            $expanded
        }
    ))
}
