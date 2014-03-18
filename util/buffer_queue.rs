/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::str::CharRange;
use extra::container::Deque;
use extra::dlist::DList;

struct Buffer {
    /// Byte position within the buffer.
    pos: uint,
    /// The buffer.
    buf: ~str,
}

impl Buffer {
    fn new(buf: ~str) -> Buffer {
        Buffer {
            pos: 0,
            buf: buf,
        }
    }
}


/// A queue of owned string buffers, which supports incrementally
/// consuming characters.
pub struct BufferQueue {
    /// Buffers to process.
    priv buffers: DList<Buffer>,

    /// Number of available characters.
    priv available: uint,
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
    pub fn push_front(&mut self, buf: ~str) {
        self.account_new(buf.as_slice());
        self.buffers.push_front(Buffer::new(buf));
        debug!("{:s}", self.dump_buffers());
    }

    /// Add a buffer to the end of the queue.
    pub fn push_back(&mut self, buf: ~str) {
        self.account_new(buf.as_slice());
        self.buffers.push_back(Buffer::new(buf));
        debug!("{:s}", self.dump_buffers());
    }

    /// Do we have at least n characters available?
    pub fn has(&mut self, n: uint) -> bool {
        self.available >= n
    }

    /// Get multiple characters, if that many are available.
    pub fn pop_front(&mut self, n: uint) -> Option<~str> {
        if !self.has(n) {
            return None;
        }
        // FIXME: this is probably pretty inefficient
        Some(self.by_ref().take(n).collect())
    }

    fn account_new(&mut self, buf: &str) {
        // FIXME: We could pass through length from the initial ~[u8] -> ~str
        // conversion, which already must re-encode or at least scan for UTF-8
        // validity.
        self.available += buf.char_len();
    }

    fn dump_buffers(&self) -> &'static str {
        debug!("BufferQueue has {:u} buffers", self.buffers.len());
        for b in self.buffers.iter() {
            debug!("    {:?}", b);
        }
        "" // for use in outer debug!()
    }
}

impl Iterator<char> for BufferQueue {
    /// Get the next character, if one is available.
    ///
    /// Because more data can arrive at any time, this can return Some(c) after
    /// it returns None.  That is allowed by the Iterator protocol, but it's
    /// unusual!
    fn next(&mut self) -> Option<char> {
        loop {
            match self.buffers.front_mut() {
                None => return None,
                Some(&Buffer { ref mut pos, ref buf }) if *pos < buf.len() => {
                    let CharRange { ch, next } = buf.char_range_at(*pos);
                    *pos = next;
                    self.available -= 1;
                    return Some(ch);
                }
                _ => ()
            }
            // Remaining case: There is a front buffer, but it's empty.
            // Do this outside the above borrow.
            self.buffers.pop_front();
        }
    }
}
