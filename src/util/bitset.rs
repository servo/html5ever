// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![macro_escape]

pub struct Bitset64 {
    pub bits: u64,
}

impl Bitset64 {
    #[inline]
    pub fn contains(self, n: u8) -> bool {
        0 != (self.bits & (1 << (n as uint)))
    }
}

macro_rules! bitset64 ( ($($e:expr),+) => (
    ::util::bitset::Bitset64 {
        bits: $( (1 << ($e as uint)) )|+
    }
))
