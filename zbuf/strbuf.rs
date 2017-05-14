use bytesbuf::BytesBuf;
use std::error;
use std::fmt;
use std::iter::FromIterator;
use std::io;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::str;
use utf8_decoder::{LossyUtf8Decoder, StrictUtf8Decoder};

/// A “zero copy” string buffer.
///
/// See [crate documentation](index.html) for an overview.
#[derive(Clone, Default, Hash, Eq, Ord)]
pub struct StrBuf(BytesBuf);

impl StrBuf {
    /// Return a new, empty, inline buffer.
    #[inline]
    pub fn new() -> Self {
        StrBuf(BytesBuf::new())
    }

    /// Return a new buffer with capacity for at least (typically more than)
    /// the given number of bytes.
    ///
    /// ## Panics
    ///
    /// Panics if the requested capacity is greater than `std::u32::MAX` (4 gigabytes).
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// assert!(StrBuf::with_capacity(17).capacity() >= 17);
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        StrBuf(BytesBuf::with_capacity(capacity))
    }

    /// Converts a bytes buffer into a string buffer.
    ///
    /// This takes `O(length)` time to check that the input is well-formed in UTF-8,
    /// and returns `Err(_)` if it is not.
    /// No heap memory is allocated or data copied, since this takes ownership of the bytes buffer.
    ///
    /// If you already know for sure that a bytes buffer is well-formed in UTF-8,
    /// consider the `unsafe` [`from_utf8_unchecked`](#method.from_utf8_unchecked) method,
    /// which takes `O(1)` time, instead.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::{StrBuf, BytesBuf};
    /// assert!(StrBuf::from_utf8(BytesBuf::from(&b"abc"[..])).is_ok());
    /// assert!(StrBuf::from_utf8(BytesBuf::from(&b"ab\x80"[..])).is_err());
    /// ```
    #[inline]
    pub fn from_utf8(bytes: BytesBuf) -> Result<Self, FromUtf8Error> {
        match str::from_utf8(&bytes) {
            Ok(_) => Ok(StrBuf(bytes)),
            Err(error) => Err(FromUtf8Error {
                bytes_buf: bytes,
                utf8_error: error,
            })
        }
    }

    /// Converts a bytes buffer into a string buffer without checking UTF-8 well-formedness.
    ///
    /// This takes `O(1)` time.
    /// No heap memory is allocated or data copied, since this takes ownership of the bytes buffer.
    ///
    /// ## Safety
    ///
    /// The given bytes buffer must be well-formed in UTF-8.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::{StrBuf, BytesBuf};
    /// let bytes_buf = BytesBuf::from(b"abc".as_ref());
    /// let str_buf = unsafe {
    ///     StrBuf::from_utf8_unchecked(bytes_buf)
    /// };
    /// assert_eq!(str_buf, "abc");
    /// ```
    #[inline]
    pub unsafe fn from_utf8_unchecked(bytes: BytesBuf) -> Self {
        StrBuf(bytes)
    }

    /// Converts a bytes buffer into a string buffer.
    ///
    /// This takes `O(length)` time to check that the input is well-formed in UTF-8,
    /// and replaces invalid byte sequences (decoding errors) with the replacement character U+FFFD.
    /// No heap memory is allocated or data copied, since this takes ownership of the bytes buffer.
    ///
    /// If you want to handle decoding errors differently,
    /// consider the [`from_utf8`](#method.from_utf8) method which returns a `Result`.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::{StrBuf, BytesBuf};
    /// assert_eq!(StrBuf::from_utf8_lossy(BytesBuf::from(&b"abc"[..])), "abc");
    /// assert_eq!(StrBuf::from_utf8_lossy(BytesBuf::from(&b"ab\x80"[..])), "ab�");
    /// ```
    pub fn from_utf8_lossy(bytes: BytesBuf) -> Self {
        let mut decoder = LossyUtf8Decoder::new();
        let mut buf: StrBuf = decoder.feed(bytes).collect();
        buf.extend(decoder.end());
        buf
    }

    /// Converts an iterator of bytes buffers into a string buffer.
    ///
    /// This takes `O(total length)` time to check that the input is well-formed in UTF-8,
    /// and returns an error at the first invalid byte sequence (decoding error).
    /// No heap memory is allocated or data copied, since this takes ownership of the bytes buffer.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let chunks = [
    ///     &[0xF0, 0x9F][..],
    ///     &[0x8E],
    ///     &[0x89],
    /// ];
    /// assert_eq!(StrBuf::from_utf8_iter(&chunks).unwrap(), "🎉");
    /// ```
    pub fn from_utf8_iter<I>(iter: I) -> Result<Self, ()>
    where I: IntoIterator, I::Item: Into<BytesBuf> {
        let mut decoder = StrictUtf8Decoder::new();
        let mut buf = StrBuf::new();
        for item in iter {
            for result in decoder.feed(item.into()) {
                buf.push_buf(&result?)
            }
        }
        decoder.end()?;
        Ok(buf)
    }

    /// Converts an iterator of bytes buffers into a string buffer.
    ///
    /// This takes `O(total length)` time to check that the input is well-formed in UTF-8,
    /// and replaces invalid byte sequences (decoding errors) with the replacement character U+FFFD.
    /// No heap memory is allocated or data copied, since this takes ownership of the bytes buffer.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let chunks = [
    ///     &[0xF0, 0x9F][..],
    ///     &[0x8E],
    ///     &[0x89, 0xF0, 0x9F],
    /// ];
    /// assert_eq!(StrBuf::from_utf8_iter_lossy(&chunks), "🎉�");
    /// ```
    pub fn from_utf8_iter_lossy<I>(iter: I) -> Self
    where I: IntoIterator, I::Item: Into<BytesBuf> {
        let mut decoder = LossyUtf8Decoder::new();
        let mut buf = StrBuf::new();
        for item in iter {
            buf.extend(decoder.feed(item.into()))
        }
        buf.extend(decoder.end());
        buf
    }

    /// Return a shared (immutable) reference to the bytes buffer representation
    /// of this string buffer.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let buf = StrBuf::from("🎉").as_bytes_buf().clone();
    /// assert_eq!(buf, [0xF0, 0x9F, 0x8E, 0x89]);
    /// ```
    #[inline]
    pub fn as_bytes_buf(&self) -> &BytesBuf {
        // This return value can be cloned to obtain a bytes buffer that shares
        // the same heap allocation as this string buffer.
        // Since that clone is shared, any mutation will cause it to re-allocate.
        // Therefore this can not be use to make a `StrBuf` not UTF-8.
        &self.0
    }

    /// Return the length of this buffer, in bytes.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// assert_eq!(StrBuf::from("🎉").len(), 4);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return whether this buffer is empty.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// assert_eq!(BytesBuf::new().is_empty(), true);
    /// assert_eq!(BytesBuf::from(b"abc".as_ref()).is_empty(), false);
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return the capacity of this buffer: the length to which it can grow
    /// without re-allocating.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// assert!(StrBuf::with_capacity(17).capacity() >= 17);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Remove the given number of bytes from the front (the start) of the buffer.
    ///
    /// This takes `O(1)` time and does not copy any heap-allocated data.
    ///
    /// ## Panics
    ///
    /// Panics if `bytes` is out of bounds or not at a `char` boundary.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let mut buf = StrBuf::from("hello");
    /// buf.pop_front(2);
    /// assert_eq!(buf, "llo");
    /// ```
    pub fn pop_front(&mut self, bytes: usize) {
        let _: &str = &self[bytes..];  // Check char boundary with a nice panic message
        self.0.pop_front(bytes)
    }

    /// Remove the given number of bytes from the back (the end) of the buffer.
    ///
    /// This takes `O(1)` time and does not copy any heap-allocated data.
    ///
    /// ## Panics
    ///
    /// Panics if `bytes` is out of bounds or not at a `char` boundary.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let mut buf = StrBuf::from("hello");
    /// buf.pop_back(2);
    /// assert_eq!(buf, "hel");
    /// ```
    pub fn pop_back(&mut self, bytes: usize) {
        let len = self.len();
        match len.checked_sub(bytes) {
            None => panic!("tried to pop {} bytes, only {} are available", bytes, len),
            Some(new_len) => self.truncate(new_len)
        }
    }

    /// Split the buffer into two at the given index.
    ///
    /// Return a new buffer that contains bytes `[at, len)`,
    /// while `self` contains bytes `[0, at)`.
    ///
    /// # Panics
    ///
    /// Panics if `at` is out of bounds or not at a `char` boundary.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let mut buf = StrBuf::from("hello");
    /// let tail = buf.split_off(2);
    /// assert_eq!(buf, "he");
    /// assert_eq!(tail, "llo");
    /// ```
    pub fn split_off(&mut self, at: usize) -> StrBuf {
        let _: &str = &self[..at];  // Check char boundary with a nice panic message
        StrBuf(self.0.split_off(at))
    }

    /// This makes the buffer empty but, unless it is shared, does not change its capacity
    ///
    /// If potentially freeing memory is preferable, consider `buf = StrBuf::empty()` instead.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let mut buf = StrBuf::from("hello");
    /// assert_eq!(buf, "hello");
    /// buf.clear();
    /// assert_eq!(buf, "");
    /// assert!(buf.capacity() > 0);
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    /// Shortens the buffer to the specified length.
    ///
    /// If `new_len` is greater than the buffer’s current length, this has no effect.
    ///
    /// ## Panics
    ///
    /// Panics if `new_len` is not at a `char` boundary.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let mut buf = StrBuf::from("hello");
    /// buf.truncate(10);
    /// assert_eq!(buf, "hello");
    /// buf.truncate(2);
    /// assert_eq!(buf, "he");
    /// ```
    pub fn truncate(&mut self, new_len: usize) {
        if new_len < self.len() {
            let _: &str = &self[..new_len];  // Check char boundary with a nice panic message
            self.0.truncate(new_len)
        }
    }

    /// Ensures that the buffer has capacity for at least (typically more than)
    /// `additional` bytes beyond its current length.
    ///
    /// This copies the data if this buffer is shared or if the existing capacity is insufficient.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let mut buf = StrBuf::from(&*"abc".repeat(10));
    /// assert!(buf.capacity() < 100);
    /// buf.reserve(100);
    /// assert!(buf.capacity() >= 130);
    /// ```
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional)
    }

    /// Extend this buffer by writing to its existing capacity.
    ///
    /// The closure is given a potentially-uninitialized mutable string slice,
    /// and returns the number of consecutive bytes written from the start of the slice.
    /// The buffer’s length is increased by that much.
    ///
    /// If `self.reserve(additional)` is called immediately before this method,
    /// the slice is at least `additional` bytes long.
    /// Without a `reserve` call the slice can be any length, including zero.
    ///
    /// This copies the existing data if there are other references to this buffer.
    ///
    /// ## Safety
    ///
    /// The closure must not *read* from the given slice, which may be uninitialized.
    /// It must initialize the `0..written` range and make it well-formed in UTF-8,
    /// where `written` is the return value.
    ///
    /// ## Panics
    ///
    /// Panics if the value returned by the closure is larger than the given closure’s length.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let mut buf = StrBuf::from("hello");
    /// buf.reserve(10);
    /// unsafe {
    ///     buf.write_to_uninitialized_tail(|uninitialized_str| {
    ///         let uninitialized_bytes = as_bytes_mut(uninitialized_str);
    ///         for byte in &mut uninitialized_bytes[..3] {
    ///             *byte = b'!'
    ///         }
    ///         3
    ///     })
    /// }
    /// assert_eq!(buf, "hello!!!");
    ///
    /// /// https://github.com/rust-lang/rust/issues/41119
    /// unsafe fn as_bytes_mut(s: &mut str) -> &mut [u8] {
    ///     ::std::mem::transmute(s)
    /// }
    /// ```
    pub unsafe fn write_to_uninitialized_tail<F>(&mut self, f: F)
    where F: FnOnce(&mut str) -> usize {
        self.0.write_to_uninitialized_tail(|uninitialized| {
            // Safety: the BytesBuf inside StrBuf is private,
            // and this module mantains UTF-8 well-formedness.
            let uninitialized_str = str_from_utf8_unchecked_mut(uninitialized);
            f(uninitialized_str)
        })
    }

    /// Extend this buffer by writing to its existing capacity.
    ///
    /// The closure is given a mutable string slice
    /// that has been overwritten with zeros (which takes `O(n)` extra time).
    /// The buffer’s length is increased by the closure’s return value.
    ///
    /// If `self.reserve(additional)` is called immediately before this method,
    /// the slice is at least `additional` bytes long.
    /// Without a `reserve` call the slice can be any length, including zero.
    ///
    /// This copies the existing data if there are other references to this buffer.
    ///
    /// ## Panics
    ///
    /// Panics if the value returned by the closure is larger than the given closure’s length,
    /// or if it is not at a `char` boundary.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let mut buf = StrBuf::from("hello");
    /// buf.reserve(10);
    /// buf.write_to_zeroed_tail(|tail| {
    ///     let tail = unsafe {
    ///         as_bytes_mut(tail)
    ///     };
    ///     for byte in &mut tail[..3] {
    ///         *byte = b'!'
    ///     }
    ///     10
    /// });
    /// assert_eq!(buf, "hello!!!\0\0\0\0\0\0\0");
    ///
    /// /// https://github.com/rust-lang/rust/issues/41119
    /// unsafe fn as_bytes_mut(s: &mut str) -> &mut [u8] {
    ///     ::std::mem::transmute(s)
    /// }
    /// ```
    pub fn write_to_zeroed_tail<F>(&mut self, f: F)
    where F: FnOnce(&mut str) -> usize {
        self.0.write_to_zeroed_tail(|tail_bytes| {
            // Safety: a sequence of zero bytes is well-formed UTF-8.
            let tail_str = unsafe {
                str_from_utf8_unchecked_mut(tail_bytes)
            };
            let additional_len = f(tail_str);
            &tail_str[..additional_len];  // Check char boundary
            additional_len
        })
    }

    /// Appends the given string slice onto the end of this buffer.
    ///
    /// This copies the existing data if this buffer is shared
    /// or if the existing capacity is insufficient.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let mut buf = StrBuf::from("hello");
    /// buf.push_str(" world!");
    /// assert_eq!(buf, "hello world!");
    /// ```
    #[inline]
    pub fn push_str(&mut self, slice: &str) {
        self.0.push_slice(slice.as_bytes())
    }

    /// Appends the given character onto the end of this buffer.
    ///
    /// This copies the existing data if this buffer is shared
    /// or if the existing capacity is insufficient.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let mut buf = StrBuf::from("hello");
    /// buf.push_char('!');
    /// assert_eq!(buf, "hello!");
    /// ```
    #[inline]
    pub fn push_char(&mut self, c: char) {
        self.push_str(c.encode_utf8(&mut [0; 4]))
    }

    /// Appends the given string buffer onto the end of this buffer.
    ///
    /// This is similar to [`push_str`](#method.push_str), but sometimes more efficient.
    ///
    /// ## Examples
    ///
    /// This allocates only once:
    ///
    /// ```
    /// # use zbuf::StrBuf;
    /// let string = "abc".repeat(20);
    /// let mut buf = StrBuf::from(&*string);
    /// let tail = buf.split_off(50);
    /// assert_eq!(buf.len(), 50);
    /// assert_eq!(tail.len(), 10);
    /// buf.push_buf(&tail);
    /// assert_eq!(buf, string);
    /// ```
    #[inline]
    pub fn push_buf(&mut self, other: &StrBuf) {
        self.0.push_buf(&other.0)
    }
}

// FIXME https://github.com/rust-lang/rust/issues/41119
#[inline]
unsafe fn str_from_utf8_unchecked_mut(v: &mut [u8]) -> &mut str {
    mem::transmute(v)
}

impl Deref for StrBuf {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        // Safety: the BytesBuf inside StrBuf is private,
        // and this module mantains UTF-8 well-formedness.
        unsafe {
            str::from_utf8_unchecked(&self.0)
        }
    }
}

/// This copies the existing data if there are other references to this buffer.
impl DerefMut for StrBuf {
    #[inline]
    fn deref_mut(&mut self) -> &mut str {
        // Safety: the BytesBuf inside StrBuf is private,
        // and this module mantains UTF-8 well-formedness.
        unsafe {
            str_from_utf8_unchecked_mut(&mut self.0)
        }
    }
}

impl AsRef<str> for StrBuf {
    #[inline]
    fn as_ref(&self) -> &str {
        self
    }
}

impl AsMut<str> for StrBuf {
    #[inline]
    fn as_mut(&mut self) -> &mut str {
        self
    }
}

impl<'a> From<&'a str> for StrBuf {
    #[inline]
    fn from(slice: &'a str) -> Self {
        StrBuf(BytesBuf::from(slice.as_bytes()))
    }
}

impl From<StrBuf> for BytesBuf {
    #[inline]
    fn from(buf: StrBuf) -> Self {
        buf.0
    }
}

impl fmt::Debug for StrBuf {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        str::fmt(self, formatter)
    }
}

impl fmt::Display for StrBuf {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        str::fmt(self, formatter)
    }
}

impl<T: AsRef<str>> PartialEq<T> for StrBuf {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        str::eq(self, other.as_ref())
    }
}

impl<T: AsRef<str>> PartialOrd<T> for StrBuf {
    #[inline]
    fn partial_cmp(&self, other: &T) -> Option<::std::cmp::Ordering> {
        str::partial_cmp(self, other.as_ref())
    }
}

impl Extend<char> for StrBuf {
    #[inline]
    fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=char> {
        for item in iter {
            self.push_char(item)
        }
    }
}

impl FromIterator<char> for StrBuf {
    #[inline]
    fn from_iter<I>(iter: I) -> Self where I: IntoIterator<Item=char> {
        let mut buf = Self::new();
        buf.extend(iter);
        buf
    }
}

impl<'a> Extend<&'a char> for StrBuf {
    #[inline]
    fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=&'a char> {
        for &item in iter {
            self.push_char(item)
        }
    }
}

impl<'a> FromIterator<&'a char> for StrBuf {
    #[inline]
    fn from_iter<I>(iter: I) -> Self where I: IntoIterator<Item=&'a char> {
        let mut buf = Self::new();
        buf.extend(iter);
        buf
    }
}

impl<'a> Extend<&'a str> for StrBuf {
    #[inline]
    fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=&'a str> {
        for item in iter {
            self.push_str(item)
        }
    }
}

impl<'a> FromIterator<&'a str> for StrBuf {
    #[inline]
    fn from_iter<I>(iter: I) -> Self where I: IntoIterator<Item=&'a str> {
        let mut buf = Self::new();
        buf.extend(iter);
        buf
    }
}

impl<'a> Extend<&'a StrBuf> for StrBuf {
    #[inline]
    fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=&'a StrBuf> {
        for item in iter {
            self.push_buf(item)
        }
    }
}

impl<'a> FromIterator<&'a StrBuf> for StrBuf {
    #[inline]
    fn from_iter<I>(iter: I) -> Self where I: IntoIterator<Item=&'a StrBuf> {
        let mut buf = Self::new();
        buf.extend(iter);
        buf
    }
}

impl Extend<StrBuf> for StrBuf {
    #[inline]
    fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=StrBuf> {
        for item in iter {
            self.push_buf(&item)
        }
    }
}

impl FromIterator<StrBuf> for StrBuf {
    #[inline]
    fn from_iter<I>(iter: I) -> Self where I: IntoIterator<Item=StrBuf> {
        let mut buf = Self::new();
        buf.extend(iter);
        buf
    }
}

impl fmt::Write for StrBuf {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        self.push_char(c);
        Ok(())
    }
}

/// The error type for [`StrBuf::from_utf8`](struct.StrBuf.html#method.from_utf8).
#[derive(Debug)]
pub struct FromUtf8Error {
    bytes_buf: BytesBuf,
    utf8_error: str::Utf8Error,
}

impl FromUtf8Error {
    pub fn as_bytes_buf(&self) -> &BytesBuf {
        &self.bytes_buf
    }

    pub fn into_bytes_buf(self) -> BytesBuf {
        self.bytes_buf
    }

    pub fn utf8_error(&self) -> str::Utf8Error {
        self.utf8_error
    }
}

impl fmt::Display for FromUtf8Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.utf8_error.fmt(formatter)
    }
}

impl error::Error for FromUtf8Error {
    fn description(&self) -> &str {
        "invalid utf-8"
    }
}

impl From<FromUtf8Error> for io::Error {
    fn from(error: FromUtf8Error) -> Self {
        Self::new(io::ErrorKind::InvalidData, error.utf8_error())
    }
}
