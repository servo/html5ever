use bytesbuf::BytesBuf;
use std::fmt;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::str;

/// A “zero copy” string buffer.
///
/// See [crate documentation](index.html) for an overview.
#[derive(Clone, Default, Hash, Eq, Ord)]
pub struct StrBuf(BytesBuf);

impl StrBuf {
    /// Return a new, empty, inline buffer.
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
    /// you can use the `unsafe` [`from_utf8_unchecked`](#method.from_utf8_unchecked") method,
    /// which takes `O(1)` time, instead.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::{StrBuf, BytesBuf};
    /// assert!(StrBuf::from_utf8(BytesBuf::from(&b"abc"[..])).is_ok());
    /// assert!(StrBuf::from_utf8(BytesBuf::from(&b"ab\x80"[..])).is_err());
    /// ```
    pub fn from_utf8(bytes: BytesBuf) -> Result<Self, BytesBuf> {
        if let Ok(_) = str::from_utf8(&bytes) {
            Ok(StrBuf(bytes))
        } else {
            Err(bytes)
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
    pub unsafe fn from_utf8_unchecked(bytes: BytesBuf) -> Self {
        StrBuf(bytes)
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
    pub fn len(&self) -> usize {
        self.0.len()
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

    /// This makes the buffer empty but, unless it is shared, does not change its capacity
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
    /// buf.truncate(2);
    /// assert_eq!(buf, "he");
    /// ```
    pub fn truncate(&mut self, new_len: usize) {
        let _: &str = &self[..new_len];  // Check char boundary with a nice panic message
        self.0.truncate(new_len)
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
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional)
    }

    /// The closure is given a potentially-uninitialized mutable string slice,
    /// and returns the number of consecutive bytes written from the start of the slice.
    /// The buffer’s length is incremented by that much.
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
    /// It must initialize the `0..written` range, where `written` is the return value.
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
        // Safety: the BytesBuf inside StrBuf is private,
        // and this module mantains UTF-8 well-formedness.
        unsafe {
            str::from_utf8_unchecked(&self.0)
        }
    }
}

/// This copies the existing data if there are other references to this buffer.
impl DerefMut for StrBuf {
    fn deref_mut(&mut self) -> &mut str {
        // Safety: the BytesBuf inside StrBuf is private,
        // and this module mantains UTF-8 well-formedness.
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

impl From<StrBuf> for BytesBuf {
    fn from(buf: StrBuf) -> Self {
        buf.0
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
