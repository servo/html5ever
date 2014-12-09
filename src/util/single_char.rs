use core::prelude::*;
use core::mem;

use iobuf::{BufSpan, Iobuf, ROIobuf};

use util::span::Span;

/// Represents a single character in an Iobuf. It contains the decoded character
/// for convenient comparison, and the Iobuf it came from.
#[deriving(Clone, Show)]
pub struct SingleChar {
    buf: ROIobuf<'static>,
}

impl SingleChar {
    #[inline(always)]
    pub fn new(buf: ROIobuf<'static>) -> SingleChar {
        SingleChar { buf: buf }
    }

    pub fn unicode_replacement() -> SingleChar {
        SingleChar {
            buf: ROIobuf::from_str("\ufffd"),
        }
    }

    pub fn null() -> SingleChar {
        SingleChar {
            buf: ROIobuf::from_str("\0"),
        }
    }

    #[inline(always)]
    pub fn into_buf(self) -> ROIobuf<'static> {
        self.buf
    }

    #[inline(always)]
    pub fn into_span(self) -> Span {
        BufSpan::from_buf(self.into_buf())
    }

    #[inline(alwyas)]
    /// Peeks at the first byte in the char. This might not be valid utf-8!
    pub fn as_u8(&self) -> u8 {
        unsafe { self.buf.unsafe_peek_be(0) }
    }

    /// This is SLOW. Don't use it. Prefer `as_u8`.
    pub fn decode_as_char(&self) -> char {
        unsafe {
            let s: &str = mem::transmute(self.buf.as_window_slice());
            s.char_at(0)
        }
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> &mut ROIobuf<'static> {
        &mut self.buf
    }
}

impl PartialEq for SingleChar {
    #[inline]
    fn eq(&self, other: &SingleChar) -> bool {
        unsafe { self.buf.as_window_slice() == other.buf.as_window_slice() }
    }
}

impl Eq for SingleChar {}

pub trait MayAppendSingleChar {
    /// Appends a "single char" to a container. Depending on implementation,
    /// it might make sense to keep either the `char` or the buffer itself.
    fn push_sc(&mut self, c: SingleChar);
}

impl MayAppendSingleChar for Span {
    #[inline(always)]
    fn push_sc(&mut self, c: SingleChar) {
        self.push(c.buf)
    }
}
