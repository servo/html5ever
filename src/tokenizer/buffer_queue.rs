// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use util::smallcharset::SmallCharSet;

use std::ascii::AsciiExt;
use std::collections::VecDeque;

use tendril::StrTendril;

pub use self::SetResult::{FromSet, NotFromSet};

/// Result from `pop_except_from`.
#[derive(PartialEq, Eq, Debug)]
pub enum SetResult {
    FromSet(char),
    NotFromSet(StrTendril),
}

/// A queue of owned string buffers, which supports incrementally
/// consuming characters.
pub struct BufferQueue {
    /// Buffers to process.
    buffers: VecDeque<StrTendril>,
}

impl BufferQueue {
    /// Create an empty BufferQueue.
    pub fn new() -> BufferQueue {
        BufferQueue {
            buffers: VecDeque::with_capacity(16),
        }
    }

    /// Add a buffer to the beginning of the queue.
    pub fn push_front(&mut self, buf: StrTendril) {
        if buf.len32() == 0 {
            return;
        }
        self.buffers.push_front(buf);
    }

    /// Add a buffer to the end of the queue.
    pub fn push_back(&mut self, buf: StrTendril) {
        if buf.len32() == 0 {
            return;
        }
        self.buffers.push_back(buf);
    }

    /// Look at the next available character, if any.
    pub fn peek(&mut self) -> Option<char> {
        // Invariant: all buffers in the queue are non-empty.
        self.buffers.front().map(|b| b.chars().next().unwrap())
    }

    /// Get the next character, if one is available.
    pub fn next(&mut self) -> Option<char> {
        let (result, now_empty) = match self.buffers.front_mut() {
            None => (None, false),
            Some(buf) => {
                let c = buf.pop_front_char().expect("empty buffer in queue");
                (Some(c), buf.is_empty())
            }
        };

        if now_empty {
            self.buffers.pop_front();
        }

        result
    }

    /// Pops and returns either a single character from the given set, or
    /// a `StrTendril` of characters none of which are in the set.  The set
    /// is represented as a bitmask and so can only contain the first 64
    /// ASCII characters.
    pub fn pop_except_from(&mut self, set: SmallCharSet) -> Option<SetResult> {
        let (result, now_empty) = match self.buffers.front_mut() {
            None => (None, false),
            Some(buf) => {
                let n = set.nonmember_prefix_len(&buf);
                if n > 0 {
                    let out;
                    unsafe {
                        out = buf.unsafe_subtendril(0, n);
                        buf.unsafe_pop_front(n);
                    }
                    (Some(NotFromSet(out)), buf.is_empty())
                } else {
                    let c = buf.pop_front_char().expect("empty buffer in queue");
                    (Some(FromSet(c)), buf.is_empty())
                }
            }
        };

        // Unborrow self for this part.
        if now_empty {
            self.buffers.pop_front();
        }

        result
    }

    // Check if the next characters are an ASCII case-insensitive match for
    // `pat`, which must be non-empty.
    //
    // If so, consume them and return Some(true).
    // If they do not match, return Some(false).
    // If not enough characters are available to know, return None.
    pub fn eat<F: Fn(&u8, &u8) -> bool>(&mut self, pat: &str, eq: F) -> Option<bool> {
        let mut buffers_exhausted = 0;
        let mut consumed_from_last = 0;
        if self.buffers.front().is_none() {
            return None;
        }

        for pattern_byte in pat.bytes() {
            if buffers_exhausted >= self.buffers.len() {
                return None;
            }
            let ref buf = self.buffers[buffers_exhausted];

            if !eq(&buf.as_bytes()[consumed_from_last], &pattern_byte) {
                return Some(false)
            }

            consumed_from_last += 1;
            if consumed_from_last >= buf.len() {
                buffers_exhausted += 1;
                consumed_from_last = 0;
            }
        }

        // We have a match. Commit changes to the BufferQueue.
        for _ in 0 .. buffers_exhausted {
            self.buffers.pop_front();
        }

        match self.buffers.front_mut() {
            None => assert_eq!(consumed_from_last, 0),
            Some(ref mut buf) => buf.pop_front(consumed_from_last as u32),
        }

        Some(true)
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use std::ascii::AsciiExt;
    use tendril::{StrTendril, SliceExt};
    use super::{BufferQueue, FromSet, NotFromSet};

    #[test]
    fn smoke_test() {
        let mut bq = BufferQueue::new();
        assert_eq!(bq.peek(), None);
        assert_eq!(bq.next(), None);

        bq.push_back("abc".to_tendril());
        assert_eq!(bq.peek(), Some('a'));
        assert_eq!(bq.next(), Some('a'));
        assert_eq!(bq.peek(), Some('b'));
        assert_eq!(bq.peek(), Some('b'));
        assert_eq!(bq.next(), Some('b'));
        assert_eq!(bq.peek(), Some('c'));
        assert_eq!(bq.next(), Some('c'));
        assert_eq!(bq.peek(), None);
        assert_eq!(bq.next(), None);
    }

    #[test]
    fn can_unconsume() {
        let mut bq = BufferQueue::new();
        bq.push_back("abc".to_tendril());
        assert_eq!(bq.next(), Some('a'));

        bq.push_front("xy".to_tendril());
        assert_eq!(bq.next(), Some('x'));
        assert_eq!(bq.next(), Some('y'));
        assert_eq!(bq.next(), Some('b'));
        assert_eq!(bq.next(), Some('c'));
        assert_eq!(bq.next(), None);
    }

    #[test]
    fn can_pop_except_set() {
        let mut bq = BufferQueue::new();
        bq.push_back("abc&def".to_tendril());
        let mut pop = || bq.pop_except_from(small_char_set!('&'));
        assert_eq!(pop(), Some(NotFromSet("abc".to_tendril())));
        assert_eq!(pop(), Some(FromSet('&')));
        assert_eq!(pop(), Some(NotFromSet("def".to_tendril())));
        assert_eq!(pop(), None);
    }

    #[test]
    fn can_eat() {
        // This is not very comprehensive.  We rely on the tokenizer
        // integration tests for more thorough testing with many
        // different input buffer splits.
        let mut bq = BufferQueue::new();
        bq.push_back("a".to_tendril());
        bq.push_back("bc".to_tendril());
        assert_eq!(bq.eat("abcd", u8::eq_ignore_ascii_case), None);
        assert_eq!(bq.eat("ax", u8::eq_ignore_ascii_case), Some(false));
        assert_eq!(bq.eat("ab", u8::eq_ignore_ascii_case), Some(true));
        assert_eq!(bq.next(), Some('c'));
        assert_eq!(bq.next(), None);
    }
}
