/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use extra::container::Deque;
use extra::dlist::DList;

/// A queue of owned string buffers, which supports incrementally
/// consuming characters.
pub struct BufferQueue {
    /// Buffers to process.
    priv buffers: DList<~str>,
}

impl BufferQueue {
    /// Create an empty BufferQueue.
    pub fn new() -> BufferQueue {
        BufferQueue {
            buffers: DList::new(),
        }
    }

    /// Add a buffer to the end of the queue.
    pub fn push_back(&mut self, buf: ~str) {
        self.buffers.push_back(buf);
    }

    /// Get the next character, if one is available.
    pub fn get_char(&mut self) -> Option<char> {
        loop {
            match self.buffers.front_mut() {
                None => return None,
                Some(ref mut buf) => {
                    if buf.len() > 0 {
                        return Some(buf.shift_char());
                    }
                }
            }
            // Remaining case: There is a front buffer, but it's empty.
            self.buffers.pop_front();
        }
    }
}
