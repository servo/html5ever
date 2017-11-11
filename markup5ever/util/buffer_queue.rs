// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The [`BufferQueue`] struct and helper types.
//!
//! This type is designed for the efficient parsing of string data, especially where many
//! significant characters are from the ascii range 0-63. This includes, for example, important
//! characters in xml/html parsing.
//!
//! Good and predictable performance is achieved by avoiding allocation where possible (a.k.a. zero
//! copy).
//!
//! [`BufferQueue`]: struct.BufferQueue.html


use std::collections::VecDeque;

use tendril::StrTendril;

pub use self::SetResult::{FromSet, NotFromSet};
use util::smallcharset::SmallCharSet;

/// Result from [`pop_except_from`] containing either a character from a [`SmallCharSet`], or a
/// string buffer of characters not from the set.
///
/// [`pop_except_from`]: struct.BufferQueue.html#method.pop_except_from
/// [`SmallCharSet`]: ../struct.SmallCharSet.html
#[derive(PartialEq, Eq, Debug)]
pub enum SetResult {
    /// A character from the `SmallCharSet`.
    FromSet(char),
    /// A block of text containing no characters from the `SmallCharSet`.
    NotFromSet(StrTendril),
}

/// A queue of owned string buffers, which supports incrementally consuming characters.
///
/// Internally it uses [`VecDeque`] and has the same complexity properties.
///
/// [`VecDeque`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html
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

    /// Returns whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    /// Get the tendril at the beginning of the queue.
    pub fn pop_front(&mut self) -> Option<StrTendril> {
        self.buffers.pop_front()
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
    pub fn peek(&self) -> Option<char> {
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
