// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use util::smallcharset::generic;

pub struct SmallCharSet {
    pub generic: generic::SmallCharSet,
    pub array: &'static [u8, ..16],
}

impl SmallCharSet {
    /// Count the number of bytes of characters at the beginning
    /// of `buf` which are not in the set.
    /// See `tokenizer::buffer_queue::pop_except_from`.
    #[allow(unsigned_negate)]
    pub fn nonmember_prefix_len(&self, buf: &[u8]) -> uint {
        // We have a choice between two instructions here.  pcmpestri takes a
        // string length in registers, while pcmpistri stops at a NULL byte.
        // pcmpestri is about half as fast because it executes additional uops
        // to load the lengths.  So we use pcmpistri, but we have to take care
        // because Rust strings aren't NULL-terminated and can contain interior
        // NULL characters.

        // First, round down the string length to a multiple of 16.
        let head_len = buf.len() & (!0xf);

        let mut neg_remainder: uint = -head_len;
        if head_len > 0 {
            let mut off: uint;
            unsafe {
                asm!("
                    movdqu ($3), %xmm0           # load the set of bytes

                 1: movdqu ($2,$0), %xmm1        # load 16 bytes of the string
                    pcmpistri $$0, %xmm1, %xmm0
                    jbe 2f                       # exit on ZF (NULL) or CF (match)
                    add $$0x10, $0
                    jnz 1b

                 2:"
                    : "=&r"(neg_remainder), "=&{ecx}"(off)
                    : "r"((buf.as_ptr() as uint) + head_len),
                      "r"(self.array as *const [u8, ..16]), "0"(neg_remainder)
                    : "xmm0", "xmm1");
            }

            // If we found a match, `neg_remainder` holds the negation (as
            // two's complement unsigned) of the number of bytes remaining,
            // including the entirety of the block with the match.  And `off`
            // contains the offset of the match within that block.
            //
            // Otherwise we found a NULL, so off == 16 no matter where the NULL
            // was.  Or we reached the end, so neg_remainder == 0.

            if (neg_remainder != 0) && (off < 16) {
                return head_len + neg_remainder + off;
            }
        }

        // If we found a NULL above, do a bytewise search on that block and we
        // will find its exact position.  If we processed the first head_len
        // bytes, do a bytewise search on the remaining 0 - 15 bytes.
        //
        // It's tempting to finish the search with pcmpestri, but this would
        // involve fetching a 16-byte block that extends past the end of the
        // string.  We don't use those bytes, but we might end up reading from
        // a page that isn't mapped, which would cause a segfault.
        //
        // Using pcmpestri would save order of 10 ns in the best case, without
        // handling the segfault issue.  And this code is only reached when
        // searching the final 0 - 15 bytes before a NULL or the end of a
        // parser input chunk.

        let pos = head_len + neg_remainder;
        pos + self.generic.nonmember_prefix_len(buf.slice_from(pos))
    }
}
