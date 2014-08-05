// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::string::String;
use std::str::CharEq;
use std::fmt::Show;

pub fn to_escaped_string<T: Show>(x: &T) -> String {
    // FIXME: don't allocate twice
    // FIXME: use std::to_str after Rust upgrade
    (format!("{}", x)).escape_default()
}

/// If `c` is an ASCII letter, return the corresponding lowercase
/// letter, otherwise None.
pub fn lower_ascii_letter(c: char) -> Option<char> {
    c.to_ascii_opt()
        .filtered(|a| a.is_alphabetic())
        .map(|a| a.to_lowercase().to_char())
}

/// Map ASCII uppercase to lowercase; preserve other characters.
pub fn lower_ascii(c: char) -> char {
    lower_ascii_letter(c).unwrap_or(c)
}

/// Is the character an ASCII alphanumeric character?
pub fn is_ascii_alnum(c: char) -> bool {
    c.to_ascii_opt().map_or(false, |a| a.is_alphanumeric())
}

/// Allocate an empty string with a small non-zero capacity.
pub fn empty_str() -> String {
    String::with_capacity(4)
}

/// ASCII whitespace characters, as defined by
/// tree construction modes that treat them specially.
pub fn is_ascii_whitespace(c: char) -> bool {
    match c {
        '\t' | '\r' | '\n' | '\x0C' | ' ' => true,
        _ => false,
    }
}

/// Count how many bytes at the beginning of the string
/// either all match or all don't match the predicate,
/// and also return whether they match.
///
/// Returns `None` on an empty string.
pub fn char_run<Pred: CharEq>(mut pred: Pred, buf: &str) -> Option<(uint, bool)> {
    let (first, rest) = buf.slice_shift_char();
    let first = unwrap_or_return!(first, None);
    let matches = pred.matches(first);

    for (idx, ch) in rest.char_indices() {
        if matches != pred.matches(ch) {
            return Some((idx + first.len_utf8_bytes(), matches));
        }
    }
    Some((buf.len(), matches))
}

#[cfg(test)]
#[allow(non_snake_case_functions)]
mod test {
    use super::*;

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

    macro_rules! test_char_run ( ($name:ident, $input:expr, $expect:expr) => (
        test_eq!($name, char_run(is_ascii_whitespace, $input), $expect)
    ))

    test_char_run!(run_empty, "", None)
    test_char_run!(run_one_t, " ", Some((1, true)))
    test_char_run!(run_one_f, "x", Some((1, false)))
    test_char_run!(run_t, "  \t  \n", Some((6, true)))
    test_char_run!(run_f, "xyzzy", Some((5, false)))
    test_char_run!(run_tf, "   xyzzy", Some((3, true)))
    test_char_run!(run_ft, "xyzzy   ", Some((5, false)))
    test_char_run!(run_tft, "   xyzzy  ", Some((3, true)))
    test_char_run!(run_ftf, "xyzzy   hi", Some((5, false)))
    test_char_run!(run_multibyte_0, "中 ", Some((3, false)))
    test_char_run!(run_multibyte_1, " 中 ", Some((1, true)))
    test_char_run!(run_multibyte_2, "  中 ", Some((2, true)))
    test_char_run!(run_multibyte_3, "   中 ", Some((3, true)))
}
