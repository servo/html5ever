// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![macro_escape]

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
    pub fn nonmember_prefix_len(&self, buf: &[u8]) -> uint {
        let mut n = 0;
        for &b in buf.iter() {
            if b >= 64 || !self.contains(b) {
                n += 1;
            } else {
                break;
            }
        }
        n
    }
}

macro_rules! generic_small_char_set ( ($($e:expr)+) => (
    ::util::smallcharset::generic::SmallCharSet {
        bits: $( (1 << ($e as uint)) )|+
    }
))
