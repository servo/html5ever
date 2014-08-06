// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! `SmallCharSet` represents a set of characters, subject to the following
//! restrictions:
//!
//! * Every character has Unicode scalar value less than 64.
//! * There are at most 16 characters in the set.
//! * `'\0'` is in the set.
//!
//! We can scan for characters in such a set using architecture-specific
//! optimizations, for example SSE 4.2 string instructions.

#![macro_escape]

#[cfg(use_arch_byte_scan)]
pub use self::arch::SmallCharSet;

#[cfg(not(use_arch_byte_scan))]
pub use self::generic::SmallCharSet;

// Architecture-specific code can fall back to the generic implementation, so
// we always compile it.
pub mod generic;

#[cfg(not(use_arch_byte_scan))]
macro_rules! small_char_set ( ($($args:tt)*) => (
    generic_small_char_set!($($args)*)
))

#[cfg(use_arch_byte_scan, target_arch="x86_64")]
#[path="x86_64.rs"]
pub mod arch;

#[cfg(test)]
mod test {
    #[test]
    fn nonmember_prefix() {
        for &c in ['&', '\0'].iter() {
            for x in range(0, 48u) {
                for y in range(0, 48u) {
                    let mut s = String::from_char(x, 'x');
                    s.push_char(c);
                    s.grow(y, 'x');
                    let set = small_char_set!('&' '\0');

                    assert_eq!(x, set.nonmember_prefix_len(s.as_bytes()));
                }
            }
        }
    }

    #[test]
    fn tricky_sse_case() {
        // A multi-byte character spanning a boundary between 16-byte
        // blocks, where the second block also contains a NULL.
        //
        // Make sure that the SSE4 code falls through to the byte-at-
        // a-time search in this case; otherwise we split the string
        // in the middle of a multi-byte character.
        let set = small_char_set!('\0');
        let s = "xxxxxxxxxxxxxx\ua66e\x00xxxxxxxxxxxxxx";
        assert!(s.slice_to(set.nonmember_prefix_len(s.as_bytes())).len() <= 17);
    }
}
