// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unused_imports)]  // for quotes

#![warn(warnings)]

use syntax::codemap::Span;
use syntax::ast::{TokenTree, TTTok};
use syntax::ast;
use syntax::ext::base::{ExtCtxt, MacResult, MacExpr};
use syntax::parse::token::{get_ident, InternedString, LIT_CHAR, IDENT};

use std::iter::Chain;
use std::slice::Items;
use std::gc::Gc;

pub fn expand(cx: &mut ExtCtxt, sp: Span, tts: &[TokenTree]) -> Box<MacResult> {
    let mut bytes: Vec<u8> = vec!();
    let mut have_null = false;

    for tt in tts.iter() {
        match *tt {
            TTTok(sp, LIT_CHAR(ch)) => {
                let n = ch as uint;
                if n == 0 {
                    have_null = true;
                } else {
                    if n > 63 {
                        bail!(cx, sp, "scalar value is above 63");
                    }
                    bytes.push(n as u8);
                }
            }
            _ => bail!(cx, sp, "expected character literal"),
        }
    }

    if !have_null {
        bail!(cx, sp, "small_char_set!() must contain '\\0'");
    }

    if bytes.len() > 15 {
        bail!(cx, sp, "small_char_set!() can contain at most 16 characters including '\\0'");
    }

    let len = bytes.len();
    bytes.grow(16 - len, &0);

    let mut literals = vec!();
    for b in bytes.move_iter() {
        literals.push_all_move(quote_tokens!(&mut *cx, $b, ));
    }

    let tts = Vec::from_slice(tts);  // FIXME: splice limitations
    MacExpr::new(quote_expr!(&mut *cx, ::util::smallcharset::arch::SmallCharSet {
        generic: generic_small_char_set!($tts),
        array: {
            static char_set_sse_array: [u8, ..16] = [ $literals ];
            &char_set_sse_array
        }
    }))
}
