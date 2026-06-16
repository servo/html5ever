// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The `BufferQueue` struct and helper types.
//!
//! This type is designed for the efficient parsing of string data, especially where many
//! significant characters are from the ascii range 0-63. This includes, for example, important
//! characters in xml/html parsing.
//!
//! Good and predictable performance is achieved by avoiding allocation where possible (a.k.a. zero
//! copy).
//!
//! [`BufferQueue`]: struct.BufferQueue.html

#[cfg(feature = "source-positions")]
use std::cell::Cell;
use std::{
    cell::{RefCell, RefMut},
    collections::VecDeque,
    mem,
};

use tendril::StrTendril;

pub use self::SetResult::{FromSet, NotFromSet};
use crate::util::smallcharset::SmallCharSet;

/// Result from [`pop_except_from`] containing either a character from a [`SmallCharSet`], or a
/// string buffer of characters not from the set.
///
/// [`pop_except_from`]: struct.BufferQueue.html#method.pop_except_from
/// [`SmallCharSet`]: ../struct.SmallCharSet.html
#[derive(PartialEq, Eq, Debug)]
pub enum SetResult {
    /// A character from the `SmallCharSet`.
    FromSet(char),
    /// A string buffer containing no characters from the `SmallCharSet`.
    NotFromSet(StrTendril),
}

/// A queue of owned string buffers, which supports incrementally consuming characters.
///
/// Internally it uses [`VecDeque`] and has the same complexity properties.
///
/// [`VecDeque`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html
#[derive(Clone, Debug)]
pub struct BufferQueue {
    /// Buffers to process.
    buffers: RefCell<VecDeque<StrTendril>>,
    /// Total number of UTF-8 bytes consumed from this queue so far.
    ///
    /// Only present when the `source-positions` feature is enabled. Used by
    /// the tokenizer to surface byte-accurate source offsets via
    /// [`TokenSink::set_current_byte`] and [`TreeSink::set_current_byte`].
    #[cfg(feature = "source-positions")]
    bytes_consumed: Cell<u64>,
}

impl Default for BufferQueue {
    /// Create an empty BufferQueue.
    #[inline]
    fn default() -> Self {
        Self {
            buffers: RefCell::new(VecDeque::with_capacity(16)),
            #[cfg(feature = "source-positions")]
            bytes_consumed: Cell::new(0),
        }
    }
}

impl BufferQueue {
    /// Returns whether the queue is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buffers.borrow().is_empty()
    }

    /// Returns the total number of UTF-8 bytes consumed from this queue.
    ///
    /// Only available when the `source-positions` feature is enabled. The
    /// value monotonically increases as characters are consumed via
    /// [`next`], [`pop_except_from`], and [`eat`]. Re-queuing bytes via
    /// [`push_front`] does **not** decrement the counter — the tokenizer
    /// uses its own `reconsume` flag for single-character look-back and
    /// never actually re-pushes bytes that were already counted.
    #[cfg(feature = "source-positions")]
    #[inline]
    pub fn bytes_consumed(&self) -> u64 {
        self.bytes_consumed.get()
    }

    /// Advance the bytes-consumed counter by `n`.
    ///
    /// Only available when the `source-positions` feature is enabled.
    /// Used by SIMD fast paths that consume bytes directly from a tendril
    /// without going through [`next`] or [`pop_except_from`].
    #[cfg(feature = "source-positions")]
    #[inline]
    pub fn advance_bytes_consumed(&self, n: u64) {
        self.bytes_consumed.set(self.bytes_consumed.get() + n);
    }

    /// Retreat the bytes-consumed counter by `n`.
    ///
    /// Only available when the `source-positions` feature is enabled. Used by
    /// tokenizer lookahead paths that consume raw bytes, then push unmatched
    /// suffix bytes back onto the queue.
    #[cfg(feature = "source-positions")]
    #[inline]
    pub fn retreat_bytes_consumed(&self, n: u64) {
        self.bytes_consumed
            .set(self.bytes_consumed.get().saturating_sub(n));
    }

    /// Get the buffer at the beginning of the queue.
    #[inline]
    pub fn pop_front(&self) -> Option<StrTendril> {
        self.buffers.borrow_mut().pop_front()
    }

    /// Add a buffer to the beginning of the queue.
    ///
    /// If the buffer is empty, it will be skipped.
    pub fn push_front(&self, buf: StrTendril) {
        if buf.len32() == 0 {
            return;
        }
        self.buffers.borrow_mut().push_front(buf);
    }

    /// Add a buffer to the end of the queue.
    ///
    /// If the buffer is empty, it will be skipped.
    pub fn push_back(&self, buf: StrTendril) {
        if buf.len32() == 0 {
            return;
        }
        self.buffers.borrow_mut().push_back(buf);
    }

    /// Look at the next available character without removing it, if the queue is not empty.
    pub fn peek(&self) -> Option<char> {
        debug_assert!(
            !self.buffers.borrow().iter().any(|el| el.len32() == 0),
            "invariant \"all buffers in the queue are non-empty\" failed"
        );
        self.buffers
            .borrow()
            .front()
            .map(|b| b.chars().next().unwrap())
    }

    /// Pops and returns either a single character from the given set, or
    /// a buffer of characters none of which are in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[macro_use] extern crate markup5ever;
    /// # #[macro_use] extern crate tendril;
    /// # fn main() {
    /// use markup5ever::buffer_queue::{BufferQueue, SetResult};
    ///
    /// let mut queue = BufferQueue::default();
    /// queue.push_back(format_tendril!(r#"<some_tag attr="text">SomeText</some_tag>"#));
    /// let set = small_char_set!(b'<' b'>' b' ' b'=' b'"' b'/');
    /// let tag = format_tendril!("some_tag");
    /// let attr = format_tendril!("attr");
    /// let attr_val = format_tendril!("text");
    /// assert_eq!(queue.pop_except_from(set), Some(SetResult::FromSet('<')));
    /// assert_eq!(queue.pop_except_from(set), Some(SetResult::NotFromSet(tag)));
    /// assert_eq!(queue.pop_except_from(set), Some(SetResult::FromSet(' ')));
    /// assert_eq!(queue.pop_except_from(set), Some(SetResult::NotFromSet(attr)));
    /// assert_eq!(queue.pop_except_from(set), Some(SetResult::FromSet('=')));
    /// assert_eq!(queue.pop_except_from(set), Some(SetResult::FromSet('"')));
    /// assert_eq!(queue.pop_except_from(set), Some(SetResult::NotFromSet(attr_val)));
    /// // ...
    /// # }
    /// ```
    pub fn pop_except_from(&self, set: SmallCharSet) -> Option<SetResult> {
        let (result, now_empty) = match self.buffers.borrow_mut().front_mut() {
            None => (None, false),
            Some(buf) => {
                let n = set.nonmember_prefix_len(buf);
                if n > 0 {
                    let out;
                    unsafe {
                        out = buf.unsafe_subtendril(0, n);
                        buf.unsafe_pop_front(n);
                    }
                    #[cfg(feature = "source-positions")]
                    self.bytes_consumed
                        .set(self.bytes_consumed.get() + out.len() as u64);
                    (Some(NotFromSet(out)), buf.is_empty())
                } else {
                    let c = buf.pop_front_char().expect("empty buffer in queue");
                    #[cfg(feature = "source-positions")]
                    self.bytes_consumed
                        .set(self.bytes_consumed.get() + c.len_utf8() as u64);
                    (Some(FromSet(c)), buf.is_empty())
                }
            },
        };

        // Unborrow self for this part.
        if now_empty {
            self.buffers.borrow_mut().pop_front();
        }

        result
    }

    /// Consume bytes matching the pattern, using a custom comparison function `eq`.
    ///
    /// Returns `Some(true)` if there is a match, `Some(false)` if there is no match, or `None` if
    /// it wasn't possible to know (more data is needed).
    ///
    /// The custom comparison function is used elsewhere to compare ascii-case-insensitively.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate markup5ever;
    /// # #[macro_use] extern crate tendril;
    /// # fn main() {
    /// use markup5ever::buffer_queue::BufferQueue;
    ///
    /// let mut queue = BufferQueue::default();
    /// queue.push_back(format_tendril!("testtext"));
    /// let test_str = "test";
    /// assert_eq!(queue.eat("test", |&a, &b| a == b), Some(true));
    /// assert_eq!(queue.eat("text", |&a, &b| a == b), Some(true));
    /// assert!(queue.is_empty());
    /// # }
    /// ```
    pub fn eat<F: Fn(&u8, &u8) -> bool>(&self, pat: &str, eq: F) -> Option<bool> {
        let mut buffers_exhausted = 0;
        let mut consumed_from_last = 0;

        self.buffers.borrow().front()?;

        for pattern_byte in pat.bytes() {
            if buffers_exhausted >= self.buffers.borrow().len() {
                return None;
            }
            let buf = &self.buffers.borrow()[buffers_exhausted];

            if !eq(&buf.as_bytes()[consumed_from_last], &pattern_byte) {
                return Some(false);
            }

            consumed_from_last += 1;
            if consumed_from_last >= buf.len() {
                buffers_exhausted += 1;
                consumed_from_last = 0;
            }
        }

        // We have a match. Commit changes to the BufferQueue.
        for _ in 0..buffers_exhausted {
            self.buffers.borrow_mut().pop_front();
        }

        match self.buffers.borrow_mut().front_mut() {
            None => assert_eq!(consumed_from_last, 0),
            Some(ref mut buf) => buf.pop_front(consumed_from_last as u32),
        }

        #[cfg(feature = "source-positions")]
        self.bytes_consumed
            .set(self.bytes_consumed.get() + pat.len() as u64);

        Some(true)
    }

    /// Get the next character if one is available, removing it from the queue.
    ///
    /// This function manages the buffers, removing them as they become empty.
    pub fn next(&self) -> Option<char> {
        let (result, now_empty) = match self.buffers.borrow_mut().front_mut() {
            None => (None, false),
            Some(buf) => {
                let c = buf.pop_front_char().expect("empty buffer in queue");
                #[cfg(feature = "source-positions")]
                self.bytes_consumed
                    .set(self.bytes_consumed.get() + c.len_utf8() as u64);
                (Some(c), buf.is_empty())
            },
        };

        if now_empty {
            self.buffers.borrow_mut().pop_front();
        }

        result
    }

    pub fn replace_with(&self, other: BufferQueue) {
        let _ = mem::replace(&mut *self.buffers.borrow_mut(), other.buffers.take());
    }

    pub fn swap_with(&self, other: &BufferQueue) {
        mem::swap(
            &mut *self.buffers.borrow_mut(),
            &mut *other.buffers.borrow_mut(),
        );
    }

    /// Return a mutable reference to the first tendril in the queue.
    pub fn peek_front_chunk_mut(&self) -> Option<RefMut<'_, StrTendril>> {
        let buffers = self.buffers.borrow_mut();
        if buffers.is_empty() {
            return None;
        }

        let front_buffer = RefMut::map(buffers, |buffers| {
            buffers.front_mut().expect("there is at least one buffer")
        });
        Some(front_buffer)
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use tendril::SliceExt;

    use super::BufferQueue;
    use super::SetResult::{FromSet, NotFromSet};

    #[test]
    fn smoke_test() {
        let bq = BufferQueue::default();
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
        let bq = BufferQueue::default();
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
        let bq = BufferQueue::default();
        bq.push_back("abc&def".to_tendril());
        let pop = || bq.pop_except_from(small_char_set!('&'));
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
        let bq = BufferQueue::default();
        bq.push_back("a".to_tendril());
        bq.push_back("bc".to_tendril());
        assert_eq!(bq.eat("abcd", u8::eq_ignore_ascii_case), None);
        assert_eq!(bq.eat("ax", u8::eq_ignore_ascii_case), Some(false));
        assert_eq!(bq.eat("ab", u8::eq_ignore_ascii_case), Some(true));
        assert_eq!(bq.next(), Some('c'));
        assert_eq!(bq.next(), None);
    }
}

#[cfg(all(test, feature = "source-positions"))]
mod test_source_positions {
    use tendril::SliceExt;

    use super::BufferQueue;
    use super::SetResult::{FromSet, NotFromSet};

    #[test]
    fn next_advances_counter_by_utf8_width() {
        let bq = BufferQueue::default();
        assert_eq!(bq.bytes_consumed(), 0);

        // ASCII: 1 byte each
        bq.push_back("abc".to_tendril());
        bq.next();
        assert_eq!(bq.bytes_consumed(), 1);
        bq.next();
        assert_eq!(bq.bytes_consumed(), 2);
        bq.next();
        assert_eq!(bq.bytes_consumed(), 3);

        // Multibyte: 'é' is 2 bytes (U+00E9, encoded as 0xC3 0xA9)
        bq.push_back("é".to_tendril());
        bq.next();
        assert_eq!(bq.bytes_consumed(), 5);
    }

    #[test]
    fn pop_except_from_bulk_advances_counter() {
        let bq = BufferQueue::default();
        // "abc" are not in the set; '&' is
        bq.push_back("abc&def".to_tendril());
        let set = small_char_set!('&');

        // Bulk NotFromSet: 3 bytes consumed
        assert_eq!(
            bq.pop_except_from(set),
            Some(NotFromSet("abc".to_tendril()))
        );
        assert_eq!(bq.bytes_consumed(), 3);

        // Single FromSet '&': 1 byte consumed
        assert_eq!(bq.pop_except_from(set), Some(FromSet('&')));
        assert_eq!(bq.bytes_consumed(), 4);

        // Bulk NotFromSet: 3 more bytes
        assert_eq!(
            bq.pop_except_from(set),
            Some(NotFromSet("def".to_tendril()))
        );
        assert_eq!(bq.bytes_consumed(), 7);
    }

    #[test]
    fn pop_except_from_multibyte_bulk_advances_by_byte_len() {
        // "café" is 5 bytes (c=1, a=1, f=1, é=2). '&' terminates the bulk.
        // Confirms NotFromSet advances by the byte length of the tendril slice,
        // not by the character count.
        let bq = BufferQueue::default();
        bq.push_back("café&".to_tendril());
        let set = small_char_set!('&');

        let result = bq.pop_except_from(set);
        assert!(matches!(result, Some(NotFromSet(_))));
        // 'c'=1 + 'a'=1 + 'f'=1 + 'é'=2 = 5 bytes
        assert_eq!(bq.bytes_consumed(), 5);
    }

    #[test]
    fn eat_advances_counter_on_match_not_on_no_match() {
        let bq = BufferQueue::default();
        bq.push_back("abcdef".to_tendril());

        // No match: counter unchanged
        assert_eq!(bq.eat("ax", u8::eq_ignore_ascii_case), Some(false));
        assert_eq!(bq.bytes_consumed(), 0);

        // Match "abc": counter advances by 3
        assert_eq!(bq.eat("abc", u8::eq_ignore_ascii_case), Some(true));
        assert_eq!(bq.bytes_consumed(), 3);

        // Match "def": counter advances by 3 more
        assert_eq!(bq.eat("def", u8::eq_ignore_ascii_case), Some(true));
        assert_eq!(bq.bytes_consumed(), 6);
    }

    #[test]
    fn push_front_does_not_decrement_counter() {
        let bq = BufferQueue::default();
        bq.push_back("abc".to_tendril());
        bq.next(); // consume 'a' → 1
        bq.next(); // consume 'b' → 2
        assert_eq!(bq.bytes_consumed(), 2);

        // Re-queue something — counter must not decrease
        bq.push_front("xy".to_tendril());
        assert_eq!(bq.bytes_consumed(), 2);

        // Consuming the re-queued bytes advances further
        bq.next(); // 'x' → 3
        bq.next(); // 'y' → 4
        assert_eq!(bq.bytes_consumed(), 4);
    }

    #[test]
    fn advance_bytes_consumed_adds_exactly() {
        let bq = BufferQueue::default();
        assert_eq!(bq.bytes_consumed(), 0);

        bq.advance_bytes_consumed(7);
        assert_eq!(bq.bytes_consumed(), 7);

        bq.advance_bytes_consumed(3);
        assert_eq!(bq.bytes_consumed(), 10);
    }
}
