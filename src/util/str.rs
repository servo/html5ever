// Copyright 2014 The HTML5 for Rust Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::strbuf::StrBuf;
use std::str::CharEq;

/// If `c` is an ASCII letter, return the corresponding lowercase
/// letter, otherwise None.
pub fn lower_ascii_letter(c: char) -> Option<char> {
    c.to_ascii_opt()
        .filtered(|a| a.is_alpha())
        .map(|a| a.to_lower().to_char())
}

/// Map ASCII uppercase to lowercase; preserve other characters.
pub fn lower_ascii(c: char) -> char {
    lower_ascii_letter(c).unwrap_or(c)
}

/// Is the character an ASCII alphanumeric character?
pub fn is_ascii_alnum(c: char) -> bool {
    c.to_ascii_opt().map_or(false, |a| a.is_alnum())
}

/// Allocate an empty string with a small non-zero capacity.
pub fn empty_str() -> StrBuf {
    StrBuf::with_capacity(4)
}

test_eq!(lower_letter_a_is_a, lower_ascii_letter('a'), Some('a'))
test_eq!(lower_letter_A_is_a, lower_ascii_letter('A'), Some('a'))
test_eq!(lower_letter_symbol_is_None, lower_ascii_letter('!'), None)
test_eq!(lower_letter_nonascii_is_None, lower_ascii_letter('\ua66e'), None)

test_eq!(lower_a_is_a, lower_ascii('a'), 'a')
test_eq!(lower_A_is_a, lower_ascii('A'), 'a')
test_eq!(lower_symbol_unchanged, lower_ascii('!'), '!')
test_eq!(lower_nonascii_unchanged, lower_ascii('\ua66e'), '\ua66e')

test_eq!(is_alnum_a, is_ascii_alnum('a'), true)
test_eq!(is_alnum_A, is_ascii_alnum('A'), true)
test_eq!(is_alnum_1, is_ascii_alnum('1'), true)
test_eq!(is_not_alnum_symbol, is_ascii_alnum('!'), false)
test_eq!(is_not_alnum_nonascii, is_ascii_alnum('\ua66e'), false)


/// ASCII whitespace characters, as defined by
/// tree construction modes that treat them specially.
pub fn is_ascii_whitespace(c: char) -> bool {
    match c {
        '\t' | '\r' | '\n' | '\x0C' | ' ' => true,
        _ => false,
    }
}

/// Split a string into runs of characters that
/// do and don't match a predicate.
pub struct Runs<'t, Pred> {
    pred: Pred,
    buf: &'t str,
}

impl<'t, Pred: CharEq> Runs<'t, Pred> {
    pub fn new(pred: Pred, buf: &'t str) -> Runs<'t, Pred> {
        Runs {
            pred: pred,
            buf: buf,
        }
    }
}

impl<'t, Pred: CharEq> Iterator<(bool, &'t str)> for Runs<'t, Pred> {
    fn next(&mut self) -> Option<(bool, &'t str)> {
        let (first, rest) = self.buf.slice_shift_char();
        let first = unwrap_or_return!(first, None);

        let matches = self.pred.matches(first);
        let len = match rest.find(|c| self.pred.matches(c) != matches) {
            Some(i) => i+1,
            None => self.buf.len(),
        };

        let run = self.buf.slice_to(len);
        self.buf = self.buf.slice_from(len);
        Some((matches, run))
    }
}

macro_rules! test_runs ( ($name:ident, $input:expr, $expect:expr) => (
    #[test]
    fn $name() {
        let mut runs = Runs::new(is_ascii_whitespace, $input);
        let result: Vec<(bool, &'static str)> = runs.collect();
        assert_eq!($expect.as_slice(), result.as_slice());
    }
))

test_runs!(runs_empty, "", [])
test_runs!(runs_one_t, " ", [(true, " ")])
test_runs!(runs_one_f, "x", [(false, "x")])
test_runs!(runs_t, "  \t  \n", [(true, "  \t  \n")])
test_runs!(runs_f, "xyzzy", [(false, "xyzzy")])
test_runs!(runs_tf, "   xyzzy", [(true, "   "), (false, "xyzzy")])
test_runs!(runs_ft, "xyzzy   ", [(false, "xyzzy"), (true, "   ")])
test_runs!(runs_tft, "   xyzzy  ", [(true, "   "), (false, "xyzzy"), (true, "  ")])
test_runs!(runs_ftf, "xyzzy   hi", [(false, "xyzzy"), (true, "   "), (false, "hi")])
