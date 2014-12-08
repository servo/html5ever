use core::prelude::*;
use core::iter;
use core::mem;
use core::str;

use collections::ring_buf::RingBuf;
use collections::string::String;

use iobuf::{BufSpan, SpanIter, Iobuf, ROIobuf};

use util::str::{Ascii, lower_ascii};

pub use self::MaybeOwnedBytes::*;

pub enum MaybeOwnedBytes<'a> {
    Slice(&'a mut [u8]),
    Owned(String),
}

/// The core type of a buffer passed into `feed`. `Buf`s are guaranteed not to
/// slice a utf-8 char in the middle. See the `iobuf` crate docs for more details
/// on using `Buf`.
pub type Buf = ROIobuf<'static>;

/// Conceptually, just a `Vec<Buf>`. If there were two buffers passed into
/// `feed`: [ "<body>hel", "lo, world!</body>" ], then the text of the body tag
/// must _span_ over multiple buffers. This allows us to concatenate strings
/// without copying the underlying buffers. See the `iobuf` crate docs for more
/// details on using `Span` (it's called `BufSpan` there).
pub type Span = BufSpan<Buf>;

/// Functions that are safe because all Spans in html5ever are valid utf-8.
pub trait ValidatedSpanUtils {
    fn iter_chars<'a>(&'a self) -> CharIterator<'a>;
    fn iter_strs<'a>(&'a self) -> StrIterator<'a>;
    fn slice_from(self, from: u32) -> Self;

    /// Writes out an ascii-lowercased version of the span into a buffer.
    /// If the buffer isn't big enough, a correctly-sized one will be allocated
    /// instead.
    fn write_into_lower<'a>(&self, buf: &'a mut [u8]) -> MaybeOwnedBytes<'a>;

    /// Runs a closure with the span as an ascii-lowercased `str` version of
    /// itself. We work very hard to prevent any heap allocations when this
    /// happens, but it might still trigger one. Helpfully, a warning will be
    /// output when that does happen.
    fn with_lower_str_copy<'a, T>(&self, f: |&str| -> T) -> T;

    fn byte_equal_slice_lower(&self, s: &[u8]) -> bool;
}

impl ValidatedSpanUtils for Span {
    fn iter_chars<'a>(&'a self) -> CharIterator<'a> {
        self.iter().flat_map(|buf| unsafe {
            let buf: &'a str = mem::transmute(buf.as_window_slice());
            buf.chars()
        })
    }

    fn iter_strs<'a>(&'a self) -> StrIterator<'a> {
        self.iter().map(|buf| unsafe {
            let buf: &'a str = mem::transmute(buf.as_window_slice());
            buf
        })
    }

    fn slice_from(self, mut from: u32) -> Span {
        let mut bufs: RingBuf<Buf> = self.into_iter().collect();
        while from > 0 {
            let needs_pop =
                match bufs.front_mut() {
                    None => break,
                    Some(ref mut buf) => {
                        match buf.advance(from) {
                            Ok(()) => {
                                from = 0;
                                buf.is_empty()
                            }
                            Err(()) => {
                                from -= buf.len();
                                true
                            }
                        }
                    }
                };

            if needs_pop {
                bufs.pop_front();
            }
        }

        // This clone makes me :(, but `RingBuf` doesn't currently support `.into_iter()`.
        bufs.iter().map(|buf| (*buf).clone()).collect()
    }

    fn write_into_lower<'a>(&self, buf: &'a mut [u8]) -> MaybeOwnedBytes<'a> {
        let len = self.count_bytes();
        if len <= buf.len() as u32 {
            for (src, dst) in self.iter_bytes().zip(buf.iter_mut()) {
                match Ascii::new(src) {
                    None    => *dst = src,
                    Some(a) => *dst = a.to_lowercase().to_u8(),
                }
            }
            Slice(buf.slice_to_mut(len as uint))
        } else {
            let mut s = String::with_capacity(len as uint);
            for c in self.iter_chars() {
                s.push(lower_ascii(c));
            }
            Owned(s)
        }
    }

    fn with_lower_str_copy<'a, T>(&self, f: |&str| -> T) -> T {
        let mut buf = [0u8, ..64];

        match self.write_into_lower(&mut buf) {
            Slice(buf) => {
                unsafe {
                    let buf_as_str: &str = mem::transmute(buf);
                    f(buf_as_str)
                }
            }
            Owned(s) => {
                h5e_warn!("Tag `{}` len={} forced us to allocate.", s, s.as_bytes().len());
                f(s.as_slice())
            }
        }
    }

    fn byte_equal_slice_lower(&self, s: &[u8]) -> bool {
        self.count_bytes_cmp(s.len()) == Ordering::Equal
        && self.iter_bytes().zip(s.iter()).all(
            |(x, &y)| lower_ascii(x as char) == lower_ascii(y as char))
    }
}

type CharIterator<'a> =
    iter::FlatMap<'static,
                  &'a ROIobuf<'static>,
                  SpanIter<'a, ROIobuf<'static>>,
                  str::Chars<'a>>;

type StrIterator<'a> =
    iter::Map<'static,
              &'a ROIobuf<'static>,
              &'a str,
              SpanIter<'a, ROIobuf<'static>>>;

#[test]
fn test_little_slice_from() {
    let span = BufSpan::from_buf(ROIobuf::from_str("hello"));

    assert_eq!(span.clone().slice_from(1), BufSpan::from_buf(ROIobuf::from_str("ello")));
    assert_eq!(span.clone().slice_from(3), BufSpan::from_buf(ROIobuf::from_str("lo")));
    assert_eq!(span.clone().slice_from(1000), BufSpan::from_buf(ROIobuf::from_str("")));
}

#[test]
fn test_big_slice_from() {
    let mut span = BufSpan::new();
    span.push(ROIobuf::from_str("hello"));
    span.push(ROIobuf::from_str(" "));
    span.push(ROIobuf::from_str("world"));
    assert_eq!(span.clone().slice_from(0), BufSpan::from_buf(ROIobuf::from_str("hello world")));
    assert_eq!(span.clone().slice_from(1), BufSpan::from_buf(ROIobuf::from_str("ello world")));
    assert_eq!(span.clone().slice_from(5), BufSpan::from_buf(ROIobuf::from_str(" world")));
    assert_eq!(span.clone().slice_from(7), BufSpan::from_buf(ROIobuf::from_str("orld")));
    assert_eq!(span.clone().slice_from(1000), BufSpan::from_buf(ROIobuf::from_str("")));
}
