/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::iter::range_inclusive;
use std::char;
use std::str;
use std::str::CharRange;
use std::slice::{Splits, Items};

#[deriving(Ord, TotalEq, TotalOrd, Show)]
pub struct DOMString {
    priv repr: Vec<u16>
}

#[deriving(Ord, TotalEq, TotalOrd, Show)]
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

fn is_lead_surrogate(x: u16) -> bool {
    (x & 0xFC00) == 0xD800
}

impl DOMString {
    pub fn empty() -> DOMString {
        DOMString { repr: Vec::new() }
    }

    pub fn from_string(s: &str) -> DOMString {
        let mut v = DOMString::empty();
        for c in s.chars() {
            v.push_char(c);
        }
        v
    }

    pub fn from_buffer(s: Vec<u16>) -> DOMString {
        DOMString { repr: s }
    }

    pub fn from_slice(s: &[u16]) -> DOMString {
        DOMString { repr: Vec::from_slice(s) }
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

    pub fn slice_from<'a>(&'a self, begin: uint) -> DOMSlice<'a> {
        DOMSlice { repr: self.repr.slice_from(begin) }
    }

    pub fn to_string(&self) -> ~str {
        str::from_utf16(self.repr.as_slice()).expect("bad UTF-16")
    }

    pub fn to_ascii_lower(&self) -> DOMString {
        self.as_slice().to_ascii_lower()
    }

    pub fn to_ascii_upper(&self) -> DOMString {
        self.as_slice().to_ascii_upper()
    }

    pub fn split_first(&self, s: u16) -> (Option<DOMString>, DOMString) {
        let mut parts = self.repr.as_slice().splitn(1, |&c| c == s);
        let fst = DOMString::from_slice(
            parts.next().expect("must have at least one part"));
        match parts.next() {
            Some(snd) => (Some(fst), DOMString::from_slice(snd)),
            None => (None, fst),
        }
    }

    pub fn truncate(&mut self, n: uint) {
        self.repr.truncate(n)
    }

    pub fn from_char(c: char) -> DOMString {
        let mut s = DOMString::empty();
        s.push_char(c);
        s
    }

    #[inline(always)] // ~4x perf on from_string
    pub fn push_char(&mut self, c: char) {
        // FIXME: is this in the std lib?
        let n = c as u32;
        if n > 0xFFFF {
            let n = n - 0x10000;
            self.repr.push((0xD800 + ((n >> 10) & 0x3FF)) as u16);
            self.repr.push((0xDC00 + (n & 0x3FF)) as u16);
        } else {
            self.repr.push(n as u16);
        }
    }

    pub fn char_len(&self) -> uint {
        self.as_slice().char_len()
    }

    pub fn char_range_at(&self, i: uint) -> CharRange {
        self.as_slice().char_range_at(i)
    }

    pub fn char_at(&self, i: uint) -> char {
        self.as_slice().char_at(i)
    }
}

impl Clone for DOMString {
    fn clone(&self) -> DOMString {
        // Use the fast to_owned defined below.
        self.as_slice().to_owned()
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

    // We can use copy_memory because u16 is Plain Old Data.
    // This is much faster than the generic Vec<T> methods.
    // See Rust bug #13472.
    pub fn to_owned(&self) -> DOMString {
        let n = self.repr.len();
        let mut vec = Vec::with_capacity(n);
        unsafe {
            vec.set_len(n);
            vec.as_mut_slice().copy_memory(self.repr);
        }
        DOMString { repr: vec }
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
        let mut replaced = Vec::new();
        for &c in self.repr.iter() {
            replaced.push_all_move(f(c).repr);
        }
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
                        .collect();
        DOMString { repr: bytes }
    }

    pub fn to_ascii_upper(&self) -> DOMString {
        let bytes = self.repr
                        .iter()
                        .map(|&b| DOMSlice::ascii_upper_char(b))
                        .collect();
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
        let mut buffer = Vec::new();
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
        if buffer.as_slice().ends_with([' ' as u16]) {
            buffer.pop();
        }
        DOMString { repr: buffer }
    }

    pub fn char_len(&self) -> uint {
        // assume valid UTF-16
        let mut lead_surrogates = 0;
        for &x in self.repr.iter() {
            if is_lead_surrogate(x) {
                lead_surrogates += 1;
            }
        }
        self.len() - lead_surrogates
    }

    pub fn char_range_at(&self, i: uint) -> CharRange {
        let x = self.repr[i];
        if is_lead_surrogate(x) {
            CharRange {
                ch: char::from_u32(0x10000
                    + ((x as u32 - 0xD800) << 10)
                    + (self.repr[i+1] as u32 - 0xDC00)).expect("bad UTF-16"),
                next: i+2,
            }
        } else {
            CharRange {
                ch: char::from_u32(x as u32).expect("bad UTF-16"),
                next: i+1,
            }
        }
    }

    pub fn char_at(&self, i: uint) -> char {
        self.char_range_at(i).ch
    }

    pub fn chars(&self) -> Chars<'a> {
        Chars {
            string: *self,
            ix: 0,
        }
    }
}

impl<'a> Eq for DOMSlice<'a> {
    fn eq(&self, other: &DOMSlice<'a>) -> bool {
        self.repr.eq(&other.repr)
    }
}

impl<'a> Add<DOMSlice<'a>, DOMString> for DOMString {
    fn add(&self, rhs: &DOMSlice<'a>) -> DOMString {
        let mut result = self.clone();
        result.push_str(*rhs);
        result
    }
}

pub struct Chars<'a> {
    priv string: DOMSlice<'a>,
    priv ix: uint,
}

impl<'a> Iterator<char> for Chars<'a> {
    #[inline]
    fn next(&mut self) -> Option<char> {
        if self.ix < self.string.len() {
            let CharRange {ch, next} = self.string.char_range_at(self.ix);
            self.ix = next;
            Some(ch)
        } else {
            None
        }
    }
}
