/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::str::CharEq;
use std::strbuf::StrBuf;

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

struct AsciiWhitespace;

impl CharEq for AsciiWhitespace {
    fn matches(&mut self, c: char) -> bool {
        match c {
            '\t' | '\r' | '\n' | '\x0C' | ' ' => true,
            _ => false,
        }
    }

    fn only_ascii(&self) -> bool {
        true
    }
}

/// Strip leading ASCII whitespace.
pub fn strip_leading_whitespace<'t>(x: &'t str) -> &'t str {
    x.trim_left_chars(AsciiWhitespace)
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

test_eq!(can_strip_space, strip_leading_whitespace("   hello"), "hello")
test_eq!(can_strip_no_space, strip_leading_whitespace("hello"), "hello")
