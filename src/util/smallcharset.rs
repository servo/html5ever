// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![macro_escape]

use core::prelude::*;

/// Represents a set of "small characters", those with Unicode scalar
/// values less than 64.
pub struct SmallCharSet {
    pub bits: u64,
}

impl SmallCharSet {
    #[inline]
    fn contains(self, n: u8) -> bool {
        0 != (self.bits & (1 << (n as uint)))
    }

    /// Count the number of bytes of characters at the beginning
    /// of `buf` which are not in the set.
    /// See `tokenizer::buffer_queue::pop_except_from`.
    pub fn nonmember_prefix_len(&self, buf: &str) -> uint {
        let mut n = 0;
        for b in buf.bytes() {
            if b >= 64 || !self.contains(b) {
                n += 1;
            } else {
                break;
            }
        }
        n
    }
}

macro_rules! small_char_set ( ($($e:expr)+) => (
    ::util::smallcharset::SmallCharSet {
        bits: $( (1 << ($e as uint)) )|+
    }
))

#[cfg(test)]
mod test {
    use core::prelude::*;
    use collections::string::String;

    #[test]
    fn nonmember_prefix() {
        for &c in ['&', '\0'].iter() {
            for x in range(0, 48u) {
                for y in range(0, 48u) {
                    let mut s = String::from_char(x, 'x');
                    s.push_char(c);
                    s.grow(y, 'x');
                    let set = small_char_set!('&' '\0');

                    assert_eq!(x, set.nonmember_prefix_len(s.as_slice()));
                }
            }
        }
    }
}
