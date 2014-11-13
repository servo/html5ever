// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::prelude::*;

use util::fast_option::{Uninit, Full, FastOption, OptValue};
use util::single_char::SingleChar;
use util::smallcharset::SmallCharSet;
use util::span::Span;
use util::str::Ascii;

use core::mem;
use core::str;
use collections::RingBuf;

use iobuf::{BufSpan, Iobuf, ROIobuf};

#[allow(dead_code)]
struct PaddedIobuf {
    buf: ROIobuf<'static>,
    #[cfg(target_word_size = "64")]
    pad: u8,
    #[cfg(target_word_size = "32")]
    pad: [u8, ..12],
}

impl PaddedIobuf {
    #[inline(always)]
    fn new(buf: ROIobuf<'static>) -> PaddedIobuf {
        unsafe {
            PaddedIobuf {
                buf: buf,
                pad: mem::uninitialized(),
            }
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> &ROIobuf<'static> {
        &self.buf
    }

    #[inline(always)]
    fn as_mut(&mut self) -> &mut ROIobuf<'static> {
        &mut self.buf
    }

    #[inline(always)]
    fn unwrap(self) -> ROIobuf<'static> {
        self.buf
    }
}

#[test]
fn test_iobuf_padded_size() {
    assert_eq!(mem::size_of::<ROIobuf<'static>>(), 24);
    assert_eq!(mem::size_of::<PaddedIobuf>(), 32);
}

/// A queue of owned string buffers, which supports incrementally
/// consuming characters.
pub struct BufferQueue {
    /// Buffers to process.
    buffers: RingBuf<PaddedIobuf>,
}

#[inline]
fn first_char_len_of_buf(buf: &ROIobuf<'static>) -> u32 {
    unsafe {
        let first_byte: u8 = buf.unsafe_peek_be(0);
        if first_byte < 0x80 { 1 } else { str::utf8_char_width(first_byte) as u32 }
    }
}

impl BufferQueue {
    /// Create an empty BufferQueue.
    pub fn new() -> BufferQueue {
        BufferQueue {
            buffers: RingBuf::with_capacity(4),
        }
    }

    /// Add a buffer to the beginning of the queue.
    ///
    /// Only push buffers that have been utf-8 validated. If the buffer _came_
    /// from the buffer queue, it's already been validated.
    pub fn push_front(&mut self, buf: ROIobuf<'static>) {
        if buf.is_empty() { return; }

        self.buffers.push_front(PaddedIobuf::new(buf));
    }

    /// Add a buffer to the end of the queue.
    /// The buffer will be validated as utf-8, panicing if it isn't.
    pub fn push_back(&mut self, buf: ROIobuf<'static>) {
        if buf.is_empty() { return; }

        if unsafe { !str::is_utf8(buf.as_window_slice()) } {
            panic!("Invalid utf-8 passed to html5ever: {}", buf);
        }

        self.buffers.push_back(PaddedIobuf::new(buf));
    }

    /// Look at the next available character, if any.
    pub fn peek(&self, dst: &mut FastOption<SingleChar>) -> OptValue {
        match self.buffers.front() {
            None => Uninit,
            Some(buf) => unsafe {
                let buf = buf.as_ref();
                let len = first_char_len_of_buf(buf);
                let mut ret_buf = (*buf).clone();
                ret_buf.unsafe_resize(len);
                dst.fill(SingleChar::new(ret_buf))
            }
        }
    }

    /// Get the next character, if one is available.
    #[inline(always)]
    pub fn next(&mut self, dst: &mut FastOption<SingleChar>) -> OptValue {
        let needs_pop =
            match self.buffers.front_mut() {
                None => return Uninit,
                Some(front_buf) => unsafe {
                    let front_buf = front_buf.as_mut();
                    let len = first_char_len_of_buf(front_buf);
                    let will_be_empty = front_buf.len() == len;
                    if dst.is_filled() {
                        let dst = dst.as_mut().as_mut();
                        dst.clone_from(front_buf);
                        dst.unsafe_resize(len);
                        front_buf.unsafe_advance(len);
                    } else {
                        dst.fill(SingleChar::new(front_buf.unsafe_split_start_at(len)));
                    }
                    will_be_empty
                }
            };

        if needs_pop {
            self.buffers.pop_front();
        }

        Full
    }

    #[inline(always)]
    pub fn next_simple(&mut self) -> Option<char> {
        let (needs_pop, result) =
            match self.buffers.front_mut() {
                None => return None,
                Some(buf) => unsafe {
                    let front_buf = buf.as_mut();
                    let s: &str = mem::transmute(front_buf.as_window_slice());
                    let str::CharRange { ch, next } = s.char_range_at(0);
                    front_buf.unsafe_advance(next as u32);
                    (front_buf.is_empty(), ch)
                }
            };

        if needs_pop {
            self.buffers.pop_front();
        }

        Some(result)
    }

    /// Pops and returns either a single character from the given set, or
    /// a `String` of characters none of which are in the set.  The set
    /// is represented as a bitmask and so can only contain the first 64
    /// ASCII characters.
    #[inline(always)]
    pub fn pop_except_from(&mut self, set: SmallCharSet, char_dst: &mut FastOption<SingleChar>, run_dst: &mut FastOption<ROIobuf<'static>>) -> (OptValue, OptValue) {
        // Load the front buffer into run_dst.
        match self.buffers.front() {
            None => return (Uninit, Uninit),
            Some(buf) => {
                let buf = buf.as_ref();
                // If the old run_dst is the same as the current buffer, just copy
                // in the new limits and bounds.
                if run_dst.is_filled() {
                    run_dst.as_mut().clone_from(buf);
                } else {
                    run_dst.fill((*buf).clone());
                }
            },
        };

        let front_buf = run_dst.as_mut();

        let front_buf_len = front_buf.len();
        let n = unsafe { set.nonmember_prefix_len(front_buf.as_window_slice()) };

        if n == 0 {
            (self.next(char_dst), Uninit)
        } else if n != front_buf_len {
            unsafe {
                front_buf.unsafe_resize(n);
                match self.buffers.front_mut() {
                    None      => {},
                    Some(buf) => buf.as_mut().unsafe_advance(n),
                };
                (Uninit, Full)
            }
        } else {
            self.buffers.pop_front();
            (Uninit, Full)
        }
    }

    // Check if the next characters are an ASCII case-insensitive match for
    // `pat`, which must be non-empty.
    //
    // If so, consume them and return the span consumed: Some(Span (non-empty)).
    // If they do not match, return Some(Span (empty)).
    // If not enough characters are available to know, return None.
    pub fn eat(&mut self, pat: &[u8]) -> Option<Span> {
        let mut buffers_exhausted  = 0u;
        let mut consumed_from_last = 0u32;

        {
            let mut buffers = self.buffers.iter().peekable();

            for &c in pat.iter() {
                let buflen = {
                    let buf = unwrap_or_return!(buffers.peek(), None).as_ref();

                    match Ascii::new(buf.peek_be(consumed_from_last).unwrap()) {
                        Some(d) if c == d.to_lowercase().to_u8() => (),
                        _ => return Some(BufSpan::new()),
                    }

                    buf.len()
                };

                // d was an ASCII character; size must be 1 byte
                consumed_from_last += 1;
                if consumed_from_last >= buflen {
                    buffers_exhausted += 1;
                    buffers.next();
                    consumed_from_last = 0;
                }
            }
        }

        let mut ret = BufSpan::new();

        // We have a match. Commit changes to the BufferQueue.
        for _ in range(0, buffers_exhausted) {
            ret.push(self.buffers.pop_front().unwrap().unwrap());
        }

        match self.buffers.front_mut() {
            None => assert_eq!(consumed_from_last, 0),
            Some(buf) => unsafe {
                let buf = buf.as_mut();
                let (begin, end) = buf.unsafe_split_at(consumed_from_last);
                *buf = end;
                ret.push(begin);
            }
        }

        Some(ret)
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use core::prelude::*;
    use collections::string::String;
    use iobuf::{BufSpan, Iobuf, ROIobuf};
    use util::fast_option::{Uninit, Full, FastOption};
    use util::smallcharset::SmallCharSet;
    use util::span::ValidatedSpanUtils;
    use super::BufferQueue;

    use self::TestSetResult::*;

    #[deriving(Eq, PartialEq, Show)]
    enum TestSetResult {
        In(u8),
        Out(String),
    }

    fn peek(bq: &mut BufferQueue) -> Option<u8> {
        let mut chr = FastOption::new();
        match bq.peek(&mut chr) {
            Uninit => None,
            Full   => Some(chr.as_ref().as_u8()),
        }
    }

    fn next(bq: &mut BufferQueue) -> Option<u8> {
        let mut chr = FastOption::new();
        match bq.next(&mut chr) {
            Uninit => None,
            Full   => Some(chr.as_ref().as_u8()),
        }
    }

    fn pop_except_from(bq: &mut BufferQueue, sc: SmallCharSet) -> Option<TestSetResult> {
        let mut chr = FastOption::new();
        let mut buf = FastOption::new();

        match bq.pop_except_from(sc, &mut chr, &mut buf) {
            (Uninit, Uninit) => {
                return None;
            }
            (Full, Uninit) => {
                Some(In(chr.as_ref().as_u8()))
            }
            (Uninit, Full) => {
                let span = BufSpan::from_buf(buf.take());
                Some(Out(span.iter_chars().collect()))
            }
            (Full, Full) => panic!("pop_except_from returned two full options"),
        }
    }

    #[test]
    fn smoke_test() {
        let mut bq = BufferQueue::new();
        let bq = &mut bq;
        assert_eq!(peek(bq), None);
        assert_eq!(next(bq), None);

        bq.push_back(ROIobuf::from_str("abc"));
        assert_eq!(peek(bq), Some(b'a'));
        assert_eq!(next(bq), Some(b'a'));
        assert_eq!(peek(bq), Some(b'b'));
        assert_eq!(peek(bq), Some(b'b'));
        assert_eq!(next(bq), Some(b'b'));
        assert_eq!(peek(bq), Some(b'c'));
        assert_eq!(next(bq), Some(b'c'));
        assert_eq!(peek(bq), None);
        assert_eq!(peek(bq), None);
    }

    #[test]
    fn can_unconsume() {
        let mut bq = BufferQueue::new();
        bq.push_back(ROIobuf::from_str("abc"));
        let bq = &mut bq;
        assert_eq!(next(bq), Some(b'a'));

        bq.push_front(ROIobuf::from_str("xy"));
        assert_eq!(next(bq), Some(b'x'));
        assert_eq!(next(bq), Some(b'y'));
        assert_eq!(next(bq), Some(b'b'));
        assert_eq!(next(bq), Some(b'c'));
        assert_eq!(next(bq), None);
    }

    #[test]
    fn can_pop_except_set() {
        let mut bq = BufferQueue::new();
        bq.push_back(ROIobuf::from_str("abc&def"));
        let pop = || pop_except_from(&mut bq, small_char_set!('&'));
        assert_eq!(pop(), Some(Out(String::from_str("abc"))));
        assert_eq!(pop(), Some(In(b'&')));
        assert_eq!(pop(), Some(Out(String::from_str("def"))));
        assert_eq!(pop(), None);
    }

    #[test]
    fn can_push_truncated() {
        let mut bq = BufferQueue::new();
        let mut buf = ROIobuf::from_str("abc");
        buf.advance(1).unwrap();
        bq.push_back(buf);
        let bq = &mut bq;
        assert_eq!(next(bq), Some(b'b'));
        assert_eq!(next(bq), Some(b'c'));
        assert_eq!(next(bq), None);
    }

    #[test]
    fn can_eat() {
        // This is not very comprehensive.  We rely on the tokenizer
        // integration tests for more thorough testing with many
        // different input buffer splits.
        let mut bq = BufferQueue::new();
        bq.push_back(ROIobuf::from_str("a"));
        bq.push_back(ROIobuf::from_str("bc"));
        let bq = &mut bq;
        assert_eq!(bq.eat(b"abcd"), None);
        assert_eq!(bq.eat(b"ax"), Some(BufSpan::new()));
        assert_eq!(bq.eat(b"ab"), Some(BufSpan::from_buf(ROIobuf::from_str("ab"))));
        assert_eq!(next(bq), Some(b'c'));
        assert_eq!(next(bq), None);
    }
}
