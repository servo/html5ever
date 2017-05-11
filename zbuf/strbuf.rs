use bytesbuf::BytesBuf;
use std::fmt;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::str;

/// A reference-counted string buffer.
#[derive(Clone, Default, Hash, Eq, Ord)]
pub struct StrBuf(BytesBuf);

impl StrBuf {
    pub fn new() -> Self {
        StrBuf(BytesBuf::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        StrBuf(BytesBuf::with_capacity(capacity))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// This does not copy any heap-allocated data.
    pub fn pop_front(&mut self, bytes: usize) {
        let _: &str = &self[bytes..];  // Check char boundary with a nice panic message
        self.0.pop_front(bytes)
    }

    /// This does not copy any data.
    pub fn pop_back(&mut self, bytes: usize) {
        let len = self.len();
        match len.checked_sub(bytes) {
            None => panic!("tried to pop {} bytes, only {} are available", bytes, len),
            Some(new_len) => self.truncate(new_len)
        }
    }

    /// This does not copy any data.
    pub fn clear(&mut self) {
        self.0.clear()
    }

    /// This does not copy any data.
    pub fn truncate(&mut self, new_len: usize) {
        let _: &str = &self[..new_len];  // Check char boundary with a nice panic message
        self.0.truncate(new_len)
    }

    /// This copies the data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional)
    }

    /// Unsafe: the closure must not *read* from the given slice, which may be uninitialized.
    ///
    /// The closure is given a potentially-uninitialized raw mutable string slice,
    /// and returns the number of consecutive bytes written from the start of the slice.
    /// The buffer’s length is incremented by that much.
    ///
    /// If `self.reserve(additional)` is called immediately before this method,
    /// the slice is at least `additional` bytes long.
    /// Without a `reserve` call the slice can be any length, including zero.
    ///
    /// This copies the existing data if there are other references to this buffer.
    pub fn write_to_uninitialized_tail<F>(&mut self, f: F) where F: FnOnce(*mut str) -> usize {
        self.0.write_to_uninitialized_tail(|uninitialized| {
            let uninitialized_str = unsafe {
                str_from_utf8_unchecked_mut(&mut *uninitialized)
            };
            f(uninitialized_str)
        })
    }

    /// This copies the existing data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub fn push_str(&mut self, slice: &str) {
        self.0.push_slice(slice.as_bytes())
    }

    /// This copies the existing data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub fn push_char(&mut self, c: char) {
        self.push_str(c.encode_utf8(&mut [0; 4]))
    }
}

// FIXME https://github.com/rust-lang/rust/issues/41119
unsafe fn str_from_utf8_unchecked_mut(v: &mut [u8]) -> &mut str {
    mem::transmute(v)
}

impl Deref for StrBuf {
    type Target = str;

    fn deref(&self) -> &str {
        unsafe {
            str::from_utf8_unchecked(&self.0)
        }
    }
}

/// This copies the existing data if there are other references to this buffer.
impl DerefMut for StrBuf {
    fn deref_mut(&mut self) -> &mut str {
        unsafe {
            str_from_utf8_unchecked_mut(&mut self.0)
        }
    }
}

impl AsRef<str> for StrBuf {
    fn as_ref(&self) -> &str {
        self
    }
}

impl AsMut<str> for StrBuf {
    fn as_mut(&mut self) -> &mut str {
        self
    }
}

impl<'a> From<&'a str> for StrBuf {
    fn from(slice: &'a str) -> Self {
        StrBuf(BytesBuf::from(slice.as_bytes()))
    }
}

impl fmt::Debug for StrBuf {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        str::fmt(self, formatter)
    }
}

impl fmt::Display for StrBuf {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        str::fmt(self, formatter)
    }
}

impl<T: AsRef<str>> PartialEq<T> for StrBuf {
    fn eq(&self, other: &T) -> bool {
        str::eq(self, other.as_ref())
    }
}

impl<T: AsRef<str>> PartialOrd<T> for StrBuf {
    fn partial_cmp(&self, other: &T) -> Option<::std::cmp::Ordering> {
        str::partial_cmp(self, other.as_ref())
    }
}
