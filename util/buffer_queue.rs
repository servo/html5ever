/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::str::CharRange;
use extra::container::Deque;
use extra::dlist::DList;

/// A queue of owned string buffers, which supports incrementally
/// consuming characters.
pub struct BufferQueue {
    /// Buffers to process.
    priv buffers: DList<~str>,

    /// Byte position within the current buffer.
    priv pos: uint,
}

impl BufferQueue {
    /// Create an empty BufferQueue.
    pub fn new() -> BufferQueue {
        BufferQueue {
            buffers: DList::new(),
            pos: 0,
        }
    }

    /// Add a buffer to the end of the queue.
    pub fn push_back(&mut self, buf: ~str) {
        if self.buffers.is_empty() {
            self.pos = 0;
        }
        self.buffers.push_back(buf);
    }
}

impl Iterator<char> for BufferQueue {
    /// Get the next character, if one is available.
    fn next(&mut self) -> Option<char> {
        loop {
            match self.buffers.front_mut() {
                None => return None,
                Some(ref mut buf) if self.pos < buf.len() => {
                    let CharRange { ch, next } = buf.char_range_at(self.pos);
                    self.pos = next;
                    return Some(ch);
                }
                _ => ()
            }
            // Remaining case: There is a front buffer, but it's empty.
            // Do this outside the above borrow.
            self.buffers.pop_front();
            self.pos = 0;
        }
    }
}
