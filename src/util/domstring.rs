/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::iter::range_inclusive;
use std::str;
use std::slice::{Splits, Items};

pub struct DOMString {
    priv repr: ~[u16]
}
pub struct DOMSlice<'a> {
    priv repr: &'a [u16]
}

static ASCII_WHITESPACE: &'static [u16] = &'static [
    ' ' as u16,
    '\t' as u16,
    '\n' as u16,
    '\r' as u16,
    '\x0C' as u16,
];

impl DOMString {
    pub fn empty() -> DOMString {
        DOMString { repr: ~[] }
    }

    pub fn from_string(s: &str) -> DOMString {
        DOMString { repr: s.to_utf16() }
    }

    pub fn from_buffer(s: ~[u16]) -> DOMString {
        DOMString { repr: s }
    }

    pub fn len(&self) -> uint {
        self.repr.len()
    }

    pub fn push_str(&mut self, s: DOMSlice) {
        self.repr.push_all(s.repr)
    }

    pub fn as_slice<'a>(&'a self) -> DOMSlice<'a> {
        DOMSlice { repr: self.repr.as_slice() }
    }

    pub fn slice<'a>(&'a self, begin: uint, end: uint) -> DOMSlice<'a> {
        DOMSlice { repr: self.repr.slice(begin, end) }
    }

    pub fn to_string(&self) -> ~str {
        str::from_utf16(self.repr).expect("bad UTF-16")
    }

    pub fn to_ascii_lower(&self) -> DOMString {
        self.as_slice().to_ascii_lower()
    }

    pub fn to_ascii_upper(&self) -> DOMString {
        self.as_slice().to_ascii_upper()
    }

    pub fn split_first(&self, s: u16) -> (Option<DOMString>, DOMString) {
        let mut parts = self.repr.splitn(1, |&c| c == s);
        let fst = DOMString::from_buffer(
            parts.next().expect("must have at least one part").to_owned());
        match parts.next() {
            Some(snd) => (Some(fst), DOMString::from_buffer(snd.to_owned())),
            None => (None, fst),
        }
    }
}

impl Clone for DOMString {
    fn clone(&self) -> DOMString {
        DOMString { repr: self.repr.clone() }
    }
}

impl Eq for DOMString {
    fn eq(&self, other: &DOMString) -> bool {
        self.repr.eq(&other.repr)
    }
}

impl<'a> DOMSlice<'a> {
    pub fn empty() -> DOMSlice<'a> {
        DOMSlice { repr: &'a [] }
    }

    pub fn len(&self) -> uint {
        self.repr.len()
    }

    pub fn iter(&self) -> Items<'a, u16> {
        self.repr.iter()
    }

    pub fn to_owned(&self) -> DOMString {
        DOMString { repr: self.repr.to_owned() }
    }

    pub fn to_string(&self) -> ~str {
        str::from_utf16(self.repr).expect("bad UTF-16")
    }

    pub fn slice(&'a self, begin: uint, end: uint) -> DOMSlice<'a> {
        DOMSlice { repr: self.repr.slice(begin, end) }
    }

    pub fn split(&self, pred: 'a |&u16| -> bool) -> Splits<'a, u16> {
        self.repr.split(pred)
    }

    pub fn split_whitespace(&self) -> Splits<'a, u16> {
        self.repr.split(|c| ASCII_WHITESPACE.contains(c))
    }

    pub fn replace(&self, f: |u16| -> DOMString) -> DOMString {
        let replaced = self.repr.flat_map(|&c| f(c).repr);
        DOMString::from_buffer(replaced)
    }

    pub fn as_vector(&self) -> &'a [u16] {
        self.repr
    }

    pub fn ascii_lower_char(b: u16) -> u16 {
        if 'A' as u16 <= b && b <= 'Z' as u16 {
            b + ('a' as u16 - 'A' as u16)
        } else {
            b
        }
    }

    pub fn ascii_upper_char(b: u16) -> u16 {
        if 'a' as u16 <= b && b <= 'z' as u16 {
            b - ('a' as u16 - 'A' as u16)
        } else {
            b
        }
    }

    pub fn to_ascii_lower(&self) -> DOMString {
        let bytes = self.repr
                        .iter()
                        .map(|&b| DOMSlice::ascii_lower_char(b))
                        .to_owned_vec();
        DOMString { repr: bytes }
    }

    pub fn to_ascii_upper(&self) -> DOMString {
        let bytes = self.repr
                        .iter()
                        .map(|&b| DOMSlice::ascii_upper_char(b))
                        .to_owned_vec();
        DOMString { repr: bytes }
    }

    pub fn eq_ignore_ascii_case(&self, other: DOMSlice) -> bool {
        self.len() == other.len() &&
        self.iter().zip(other.iter()).all(|(&s, &o)| {
            s == o ||
            DOMSlice::ascii_lower_char(s) == DOMSlice::ascii_lower_char(o)
        })
    }

    pub fn starts_with(&self, other: DOMSlice) -> bool {
        self.repr.starts_with(other.repr)
    }

    pub fn ends_with(&self, other: DOMSlice) -> bool {
        self.repr.ends_with(other.repr)
    }

    pub fn contains(&self, needle: DOMSlice) -> bool {
        let (m, n) = (self.len(), needle.len());
        m <= n &&
        range_inclusive(0, m - n).any(|i| self.slice(i, m).starts_with(needle))
    }

    pub fn compressed_whitespace(&self) -> DOMString {
        fn is_whitespace(c: u16) -> bool {
            ASCII_WHITESPACE.contains(&c)
        }

        let mut i = 0u;
        while i < self.len() && is_whitespace(self.repr[i]) {
            i += 1;
        }

        let slice = self.repr.slice_from(i);
        let mut last_whitespace = false;
        let mut buffer = ~[];
        for &c in slice.iter() {
            if is_whitespace(c) {
                if !last_whitespace {
                    last_whitespace = true;
                    buffer.push(' ' as u16)
                }
            } else {
                buffer.push(c)
            }
        }
        if buffer.ends_with([' ' as u16]) {
            buffer.pop();
        }
        DOMString { repr: buffer }
    }
}

impl<'a> Eq for DOMSlice<'a> {
    fn eq(&self, other: &DOMSlice<'a>) -> bool {
        self.repr.eq(&other.repr)
    }
}

impl Equiv<DOMString> for DOMString {
    fn equiv(&self, other: &DOMString) -> bool {
        self.repr.equiv(&other.repr)
    }
}

impl<'a> Equiv<DOMString> for DOMSlice<'a> {
    fn equiv(&self, other: &DOMString) -> bool {
        self.repr.equiv(&other.repr)
    }
}

impl<'a> Add<DOMSlice<'a>, DOMString> for DOMString {
    fn add(&self, rhs: &DOMSlice<'a>) -> DOMString {
        let mut result = self.clone();
        result.push_str(*rhs);
        result
    }
}

pub fn null_str_as_empty(s: &Option<DOMString>) -> DOMString {
    // We don't use map_default because it would allocate ~"" even for Some.
    match *s {
        Some(ref s) => s.clone(),
        None => DOMString::empty(),
    }
}

pub fn null_str_as_empty_ref<'a>(s: &'a Option<DOMString>) -> DOMSlice<'a> {
    match *s {
        Some(ref s) => s.as_slice(),
        None => DOMSlice { repr: &'a [] },
    }
}
