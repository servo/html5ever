// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use util::smallcharset::SmallCharSet;

use std::str::CharRange;
use std::string::String;
use std::collections::Deque;
use std::collections::dlist::DList;

struct Buffer {
    /// Byte position within the buffer.
    pub pos: uint,
    /// The buffer.
    pub buf: String,
}

/// Result from `pop_except_from`.
#[deriving(PartialEq, Eq, Show)]
pub enum SetResult {
    FromSet(char),
    NotFromSet(String),
}

/// A queue of owned string buffers, which supports incrementally
/// consuming characters.
pub struct BufferQueue {
    /// Buffers to process.
    buffers: DList<Buffer>,

    /// Number of available characters.
    available: uint,
}

impl BufferQueue {
    /// Create an empty BufferQueue.
    pub fn new() -> BufferQueue {
        BufferQueue {
            buffers: DList::new(),
            available: 0,
        }
    }

    /// Add a buffer to the beginning of the queue.
    pub fn push_front(&mut self, buf: String) {
        if buf.len() == 0 {
            return;
        }
        self.account_new(buf.as_slice());
        self.buffers.push_front(Buffer {
            pos: 0,
            buf: buf,
        });
    }

    /// Add a buffer to the end of the queue.
    /// 'pos' can be non-zero to remove that many characters
    /// from the beginning.
    pub fn push_back(&mut self, buf: String, pos: uint) {
        if pos >= buf.len() {
            return;
        }
        self.account_new(buf.as_slice());
        self.buffers.push_back(Buffer {
            pos: pos,
            buf: buf,
        });
    }

    /// Do we have at least n characters available?
    pub fn has(&mut self, n: uint) -> bool {
        self.available >= n
    }

    /// Get multiple characters, if that many are available.
    pub fn pop_front(&mut self, n: uint) -> Option<String> {
        if !self.has(n) {
            return None;
        }
        // FIXME: this is probably pretty inefficient
        Some(self.by_ref().take(n).collect())
    }

    /// Look at the next available character, if any.
    pub fn peek(&mut self) -> Option<char> {
        match self.buffers.front() {
            Some(&Buffer { pos, ref buf }) => Some(buf.as_slice().char_at(pos)),
            None => None,
        }
    }

    /// Pops and returns either a single character from the given set, or
    /// a `String` of characters none of which are in the set.  The set
    /// is represented as a bitmask and so can only contain the first 64
    /// ASCII characters.
    pub fn pop_except_from(&mut self, set: SmallCharSet) -> Option<SetResult> {
        let (result, now_empty) = match self.buffers.front_mut() {
            Some(&Buffer { ref mut pos, ref buf }) => {
                let n = set.nonmember_prefix_len(buf.as_bytes().slice_from(*pos));
                if n > 0 {
                    let new_pos = *pos + n;
                    let out = buf.as_slice().slice(*pos, new_pos).to_string();
                    *pos = new_pos;
                    self.available -= n;
                    (Some(NotFromSet(out)), new_pos >= buf.len())
                } else {
                    let CharRange { ch, next } = buf.as_slice().char_range_at(*pos);
                    *pos = next;
                    self.available -= 1;
                    (Some(FromSet(ch)), next >= buf.len())
                }
            }
            _ => (None, false),
        };

        // Unborrow self for this part.
        if now_empty {
            self.buffers.pop_front();
        }

        result
    }

    fn account_new(&mut self, buf: &str) {
        // FIXME: We could pass through length from the initial [u8] -> String
        // conversion, which already must re-encode or at least scan for UTF-8
        // validity.
        self.available += buf.char_len();
    }
}

impl Iterator<char> for BufferQueue {
    /// Get the next character, if one is available.
    ///
    /// Because more data can arrive at any time, this can return Some(c) after
    /// it returns None.  That is allowed by the Iterator protocol, but it's
    /// unusual!
    fn next(&mut self) -> Option<char> {
        let (result, now_empty) = match self.buffers.front_mut() {
            None => (None, false),
            Some(&Buffer { ref mut pos, ref buf }) => {
                let CharRange { ch, next } = buf.as_slice().char_range_at(*pos);
                *pos = next;
                self.available -= 1;
                (Some(ch), next >= buf.len())
            }
        };

        if now_empty {
            self.buffers.pop_front();
        }

        result
    }
}

#[cfg(test)]
#[allow(non_snake_case_functions)]
mod test {
    use super::*; // public items

    #[test]
    fn smoke_test() {
        let mut bq = BufferQueue::new();
        assert_eq!(bq.has(1), false);
        assert_eq!(bq.peek(), None);
        assert_eq!(bq.next(), None);

        bq.push_back("abc".to_string(), 0);
        assert_eq!(bq.has(1), true);
        assert_eq!(bq.has(3), true);
        assert_eq!(bq.has(4), false);

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
    fn can_pop_front() {
        let mut bq = BufferQueue::new();
        bq.push_back("abc".to_string(), 0);

        assert_eq!(bq.pop_front(2), Some("ab".to_string()));
        assert_eq!(bq.peek(), Some('c'));
        assert_eq!(bq.pop_front(2), None);
        assert_eq!(bq.next(), Some('c'));
        assert_eq!(bq.next(), None);
    }

    #[test]
    fn can_unconsume() {
        let mut bq = BufferQueue::new();
        bq.push_back("abc".to_string(), 0);
        assert_eq!(bq.next(), Some('a'));

        bq.push_front("xy".to_string());
        assert_eq!(bq.next(), Some('x'));
        assert_eq!(bq.next(), Some('y'));
        assert_eq!(bq.next(), Some('b'));
        assert_eq!(bq.next(), Some('c'));
        assert_eq!(bq.next(), None);
    }

    #[test]
    fn can_pop_except_set() {
        let mut bq = BufferQueue::new();
        bq.push_back("abc&def".to_string(), 0);
        let pop = || bq.pop_except_from(small_char_set!('\0' '&'));
        assert_eq!(pop(), Some(NotFromSet("abc".to_string())));
        assert_eq!(pop(), Some(FromSet('&')));
        assert_eq!(pop(), Some(NotFromSet("def".to_string())));
        assert_eq!(pop(), None);
    }

    #[test]
    fn can_push_truncated() {
        let mut bq = BufferQueue::new();
        bq.push_back("abc".to_string(), 1);
        assert_eq!(bq.next(), Some('b'));
        assert_eq!(bq.next(), Some('c'));
        assert_eq!(bq.next(), None);
    }
}
