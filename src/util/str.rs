// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::prelude::*;

use core::str::CharEq;
use collections::vec::Vec;
use collections::string;
use collections::string::String;

#[cfg(not(for_c))]
use core::fmt::Show;

#[cfg(not(for_c))]
pub fn to_escaped_string<T: Show>(x: &T) -> String {
    use std::string::ToString;
    use collections::str::StrAllocating;

    // FIXME: don't allocate twice
    x.to_string().escape_default()
}

// FIXME: The ASCII stuff is largely copied from std::ascii
// (see rust-lang/rust#16801).

pub static ASCII_LOWER_MAP: [u8, ..256] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
    0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
    0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
    b' ', b'!', b'"', b'#', b'$', b'%', b'&', b'\'',
    b'(', b')', b'*', b'+', b',', b'-', b'.', b'/',
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7',
    b'8', b'9', b':', b';', b'<', b'=', b'>', b'?',
    b'@',

          b'a', b'b', b'c', b'd', b'e', b'f', b'g',
    b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o',
    b'p', b'q', b'r', b's', b't', b'u', b'v', b'w',
    b'x', b'y', b'z',

                      b'[', b'\\', b']', b'^', b'_',
    b'`', b'a', b'b', b'c', b'd', b'e', b'f', b'g',
    b'h', b'i', b'j', b'k', b'l', b'm', b'n', b'o',
    b'p', b'q', b'r', b's', b't', b'u', b'v', b'w',
    b'x', b'y', b'z', b'{', b'|', b'}', b'~', 0x7f,
    0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f,
    0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97,
    0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0x9f,
    0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad, 0xae, 0xaf,
    0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7,
    0xb8, 0xb9, 0xba, 0xbb, 0xbc, 0xbd, 0xbe, 0xbf,
    0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7,
    0xc8, 0xc9, 0xca, 0xcb, 0xcc, 0xcd, 0xce, 0xcf,
    0xd0, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7,
    0xd8, 0xd9, 0xda, 0xdb, 0xdc, 0xdd, 0xde, 0xdf,
    0xe0, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7,
    0xe8, 0xe9, 0xea, 0xeb, 0xec, 0xed, 0xee, 0xef,
    0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7,
    0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd, 0xfe, 0xff,
];

#[deriving(Clone, Copy, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub struct Ascii {
    chr: u8,
}

impl Ascii {
    pub fn to_char(self) -> char {
        self.chr as char
    }

    #[inline]
    pub fn is_alphabetic(&self) -> bool {
        (self.chr >= 0x41 && self.chr <= 0x5A) || (self.chr >= 0x61 && self.chr <= 0x7A)
    }

    #[inline]
    pub fn is_digit(&self) -> bool {
        self.chr >= 0x30 && self.chr <= 0x39
    }

    #[inline]
    pub fn is_alphanumeric(&self) -> bool {
        self.is_alphabetic() || self.is_digit()
    }

    #[inline]
    pub fn to_lowercase(self) -> Ascii {
        Ascii { chr: ASCII_LOWER_MAP[self.chr as uint] }
    }

    #[inline]
    pub fn eq_ignore_case(self, other: Ascii) -> bool {
        ASCII_LOWER_MAP[self.chr as uint] == ASCII_LOWER_MAP[other.chr as uint]
    }
}

pub trait AsciiCast {
    fn to_ascii_opt(&self) -> Option<Ascii>;
}

impl AsciiCast for char {
    fn to_ascii_opt(&self) -> Option<Ascii> {
        let n = *self as uint;
        if n < 0x80 {
            Some(Ascii { chr: n as u8 })
        } else {
            None
        }
    }
}

pub trait AsciiExt<T> {
    fn to_ascii_lower(&self) -> T;
    fn eq_ignore_ascii_case(&self, other: Self) -> bool;
}

impl<'a> AsciiExt<Vec<u8>> for &'a [u8] {
    #[inline]
    fn to_ascii_lower(&self) -> Vec<u8> {
        self.iter().map(|&byte| ASCII_LOWER_MAP[byte as uint]).collect()
    }

    #[inline]
    fn eq_ignore_ascii_case(&self, other: &[u8]) -> bool {
        self.len() == other.len() && self.iter().zip(other.iter()).all(
            |(byte_self, byte_other)| {
                ASCII_LOWER_MAP[*byte_self as uint] ==
                    ASCII_LOWER_MAP[*byte_other as uint]
            }
        )
    }
}

impl<'a> AsciiExt<String> for &'a str {
    #[inline]
    fn to_ascii_lower(&self) -> String {
        // Vec<u8>::to_ascii_lower() preserves the UTF-8 invariant.
        unsafe { string::raw::from_utf8(self.as_bytes().to_ascii_lower()) }
    }

    #[inline]
    fn eq_ignore_ascii_case(&self, other: &str) -> bool {
        self.as_bytes().eq_ignore_ascii_case(other.as_bytes())
    }
}

/// If `c` is an ASCII letter, return the corresponding lowercase
/// letter, otherwise None.
pub fn lower_ascii_letter(c: char) -> Option<char> {
    match c.to_ascii_opt() {
        Some(ref a) if a.is_alphabetic() => Some(a.to_lowercase().to_char()),
        _ => None,
    }
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
    let (first, rest) = unwrap_or_return!(buf.slice_shift_char(), None);
    let matches = pred.matches(first);

    for (idx, ch) in rest.char_indices() {
        if matches != pred.matches(ch) {
            return Some((idx + first.len_utf8_bytes(), matches));
        }
    }
    Some((buf.len(), matches))
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use core::prelude::*;
    use super::{char_run, is_ascii_whitespace, is_ascii_alnum, lower_ascii, lower_ascii_letter};

    test_eq!(lower_letter_a_is_a, lower_ascii_letter('a'), Some('a'))
    test_eq!(lower_letter_A_is_a, lower_ascii_letter('A'), Some('a'))
    test_eq!(lower_letter_symbol_is_None, lower_ascii_letter('!'), None)
    test_eq!(lower_letter_nonascii_is_None, lower_ascii_letter('\u{a66e}'), None)

    test_eq!(lower_a_is_a, lower_ascii('a'), 'a')
    test_eq!(lower_A_is_a, lower_ascii('A'), 'a')
    test_eq!(lower_symbol_unchanged, lower_ascii('!'), '!')
    test_eq!(lower_nonascii_unchanged, lower_ascii('\u{a66e}'), '\u{a66e}')

    test_eq!(is_alnum_a, is_ascii_alnum('a'), true)
    test_eq!(is_alnum_A, is_ascii_alnum('A'), true)
    test_eq!(is_alnum_1, is_ascii_alnum('1'), true)
    test_eq!(is_not_alnum_symbol, is_ascii_alnum('!'), false)
    test_eq!(is_not_alnum_nonascii, is_ascii_alnum('\u{a66e}'), false)

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
