use heap_data::{TaggedPtr, HeapAllocation};
use std::fmt;
use std::hash;
use std::iter::FromIterator;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::ptr;
use std::slice;
use u32_to_usize;
use usize_to_u32;

/// A “zero copy” bytes buffer.
///
/// See [crate documentation](index.html) for an overview.
#[derive(Clone)]
pub struct BytesBuf(Inner);

/// The memory representation of `Inner` is one of two cases:
/// the heap-allocated case and the inline case.
/// The inline case is used for small buffers
/// (15 bytes or less on 64-bit platforms, 11 bytes or less on 32-bit).
///
/// * In the heap-allocated case, the fields are what their type says:
///   a pointer and two integers.
///   Because of the memory alignment requirement of `HeapData`,
///   `ptr`’s lower two bits are zero.
///
/// * In the inline-case, these same lower two bits of `ptr` are set to a non-zero value.
///   This serves as a tag to distinguish the two cases.
///   The rest of `ptr`’s lower byte stores the buffer’s length.
///   (4 bits would suffice for this since that length can not be more than 15.)
///   Finally the rest of `Inner`’s bytes are used to store the buffer’s content, inline.
///
///   To make this inline buffer an uninterrupted slice,
///   the metadata byte (that contains the tag and the length, `ptr`’s lower byte)
///   must be at an "edge" of `Inner`.
///   For this reason we use a different layout
///   on little-endian platforms (metadata byte at the start of `Inner`)
///   and on big-endian platforms (metadata byte at the end of `Inner`).
#[cfg(target_endian = "little")]
#[derive(Clone)]
#[repr(C)]  // Don’t re-order fields
struct Inner {
    ptr: TaggedPtr,
    start: u32,
    len: u32,
}

#[cfg(target_endian = "big")]
#[derive(Clone)]
#[repr(C)]  // Don’t re-order fields
struct Inner {
    start: u32,
    len: u32,
    ptr: TaggedPtr,
}

/// Offset from the start of `Inner` to the start of inline buffer data.
/// On little-endian the metadata byte is at the start, so inline data starts after that.
/// On big-endian the metadata byte is at the end of `Inner`.
#[cfg(target_endian = "little")]
const INLINE_DATA_OFFSET_BYTES: isize = 1;

#[cfg(target_endian = "big")]
const INLINE_DATA_OFFSET_BYTES: isize = 0;

const INLINE_LENGTH_MASK: usize = 0b_1111_1100;
const INLINE_LENGTH_OFFSET_BITS: usize = 2;

fn inline_length(metadata: usize) -> usize {
    (metadata & INLINE_LENGTH_MASK) >> INLINE_LENGTH_OFFSET_BITS
}

fn set_inline_length(metadata: usize, new_len: usize) -> usize {
    debug_assert!(new_len <= INLINE_CAPACITY);
    let without_len = metadata & !INLINE_LENGTH_MASK;
    let with_new_len = without_len | (new_len << INLINE_LENGTH_OFFSET_BITS);
    with_new_len
}

/// `size_of::<Inner>()`, except `size_of` can not be used in a constant expression.
#[cfg(target_pointer_width = "32")]
const SIZE_OF_INNER: usize = 4 + 4 + 4;

#[cfg(target_pointer_width = "64")]
const SIZE_OF_INNER: usize = 8 + 4 + 4;

#[allow(dead_code)]
unsafe fn static_assert(x: Inner) {
    mem::transmute::<Inner, [u8; SIZE_OF_INNER]>(x);  // Assert that SIZE_OF_INNER is correct
}

/// How many bytes can be stored inline, leaving one byte of metadata.
const INLINE_CAPACITY: usize = SIZE_OF_INNER - 1;

impl BytesBuf {
    /// Return a new, empty, inline buffer.
    #[inline]
    pub fn new() -> Self {
        let metadata = 0;  // Includes bits for `length = 0`
        BytesBuf(Inner {
            ptr: TaggedPtr::new_inline_data(metadata),
            start: 0,
            len: 0,
        })
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
    /// # use zbuf::BytesBuf;
    /// assert!(BytesBuf::with_capacity(17).capacity() >= 17);
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        if capacity <= INLINE_CAPACITY {
            Self::new()
        } else {
            BytesBuf(Inner {
                ptr: TaggedPtr::allocate(capacity),
                start: 0,
                len: 0,
            })
        }
    }

    #[inline]
    fn as_allocated(&self) -> Result<&HeapAllocation, usize> {
        self.0.ptr.as_allocated()
    }

    /// Return the length of this buffer, in bytes.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// assert_eq!(BytesBuf::from("🎉".as_bytes()).len(), 4);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        match self.as_allocated() {
            Ok(_) => u32_to_usize(self.0.len),
            Err(metadata) => inline_length(metadata),
        }
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
        self.len() == 0
    }

    fn data_and_uninitialized_tail(&mut self) -> (&mut [u8], *mut [u8]) {
        if self.0.ptr.is_shared_allocation() {
            *self = {
                let slice: &[u8] = self;
                Self::from(slice)
            }
        }

        if let Ok(metadata) = self.0.ptr.get_inline_data() {
            let len = inline_length(metadata);
            let struct_ptr: *mut Inner = &mut self.0;
            // Safety relies on INLINE_DATA_OFFSET_BYTES and INLINE_CAPACITY being correct
            // to give a slice within the memory layout of `Inner`.
            // Inline data is never uninitialized.
            unsafe {
                let data_ptr = (struct_ptr as *mut u8).offset(INLINE_DATA_OFFSET_BYTES);
                let inline = slice::from_raw_parts_mut(data_ptr, INLINE_CAPACITY);
                let (initialized, tail) = inline.split_at_mut(len);
                return (initialized, tail)
            }
        }

        let heap_allocation = self.0.ptr.as_owned_allocated_mut()
            .expect("expected owned allocation");

        let start = u32_to_usize(self.0.start);
        let len = u32_to_usize(self.0.len);
        let data = heap_allocation.data_mut();
        // Safety: the start..(start+len) range is known to be initialized.
        unsafe {
            let (initialized, tail) = (*data)[start..].split_at_mut(len);
            return (initialized, tail)
        }
    }

    /// Return the capacity of this buffer: the length to which it can grow
    /// without re-allocating.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// assert!(BytesBuf::with_capacity(17).capacity() >= 17);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        if let Ok(heap_allocation) = self.as_allocated() {
            let capacity = if heap_allocation.is_owned() {
                heap_allocation.data_capacity().checked_sub(self.0.start)
                    .expect("data_capacity < start ??")
            } else {
                // This heap data is shared, we can’t write to it.
                // So we want `self.reserve(additional)` to reallocate if `additional > 0`,
                // but at the same time avoid `self.capacity() < self.len()`
                self.0.len
            };
            u32_to_usize(capacity)
        } else {
            INLINE_CAPACITY
        }
    }

    /// Remove the given number of bytes from the front (the start) of the buffer.
    ///
    /// This takes `O(1)` time and does not copy any heap-allocated data.
    ///
    /// ## Panics
    ///
    /// Panics if `bytes` is out of bounds.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// let mut buf = BytesBuf::from(b"hello".as_ref());
    /// buf.pop_front(2);
    /// assert_eq!(buf, b"llo");
    /// ```
    pub fn pop_front(&mut self, bytes: usize) {
        if let Ok(_) = self.as_allocated() {
            let bytes = usize_to_u32(bytes);
            match self.0.len.checked_sub(bytes) {
                None => panic!("tried to pop {} bytes, only {} are available", bytes, self.0.len),
                Some(new_len) => {
                    self.0.len = new_len;
                    self.0.start = self.0.start.checked_add(bytes).expect("overflow");
                }
            }
        } else {
            // `self` was small enough to be inline, so the new buffer will too.
            *self = Self::from(&self[bytes..])
        }
    }

    /// Remove the given number of bytes from the back (the end) of the buffer.
    ///
    /// This takes `O(1)` time and does not copy any heap-allocated data.
    ///
    /// ## Panics
    ///
    /// Panics if `bytes` is out of bounds.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// let mut buf = BytesBuf::from(b"hello".as_ref());
    /// buf.pop_back(2);
    /// assert_eq!(buf, b"hel");
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
    /// Panics if `at` is out of bounds.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// let mut buf = BytesBuf::from(b"hello".as_ref());
    /// let tail = buf.split_off(2);
    /// assert_eq!(buf, b"he");
    /// assert_eq!(tail, b"llo");
    /// ```
    pub fn split_off(&mut self, at: usize) -> BytesBuf {
        let mut tail;
        if let Ok(_) = self.as_allocated() {
            let _: &[u8] = &self[at..];  // Check bounds
            let at = usize_to_u32(at);
            tail = self.clone();
            tail.0.start += at;
            tail.0.len -= at;
        } else {
            tail = Self::from(&self[at..])
        }
        self.truncate(at);
        tail
    }

    /// This makes the buffer empty but, unless it is shared, does not change its capacity.
    ///
    /// If potentially freeing memory is preferable, consider `buf = BytesBuf::empty()` instead.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// let mut buf = BytesBuf::from(b"hello".as_ref());
    /// assert_eq!(buf, b"hello");
    /// buf.clear();
    /// assert_eq!(buf, b"");
    /// assert!(buf.capacity() > 0);
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.truncate(0)
    }

    /// Shortens the buffer to the specified length.
    ///
    /// If `new_len` is greater than the buffer’s current length, this has no effect.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// let mut buf = BytesBuf::from(b"hello".as_ref());
    /// buf.truncate(2);
    /// assert_eq!(buf, b"he");
    /// ```
    pub fn truncate(&mut self, new_len: usize) {
        if new_len < self.len() {
            // Safety: 0..len is known to be initialized
            unsafe {
                self.set_len(new_len)
            }
        }
    }

    /// Unsafe: 0..new_len data must be initialized
    unsafe fn set_len(&mut self, new_len: usize) {
        match self.as_allocated() {
            Ok(_) => {
                self.0.len = usize_to_u32(new_len)
            }
            Err(metadata) => {
                self.0.ptr = TaggedPtr::new_inline_data(set_inline_length(metadata, new_len))
            }
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
    /// # use zbuf::BytesBuf;
    /// let mut buf = BytesBuf::from("abc".repeat(10).as_bytes());
    /// assert!(buf.capacity() < 100);
    /// buf.reserve(100);
    /// assert!(buf.capacity() >= 130);
    /// ```
    pub fn reserve(&mut self, additional: usize) {
        let new_capacity = self.len().checked_add(additional).expect("overflow");
        // self.capacity() already caps at self.len() for shared (not owned) heap-allocated buffers.
        if new_capacity > self.capacity() {
            let mut copy = Self::with_capacity(new_capacity);
            // Safety: copy_into_prefix’s contract
            unsafe {
                copy.write_to_uninitialized_tail(|uninit| copy_into_prefix(self, uninit))
            }
            *self = copy
        }
    }

    /// Extend this buffer by writing to its existing capacity.
    ///
    /// The closure is given a potentially-uninitialized mutable bytes slice,
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
    /// It must initialize the `0..written` range, where `written` is the return value.
    ///
    /// ## Panics
    ///
    /// Panics if the value returned by the closure is larger than the given closure’s length.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// let mut buf = BytesBuf::from(b"hello".as_ref());
    /// buf.reserve(10);
    /// unsafe {
    ///     buf.write_to_uninitialized_tail(|uninitialized| {
    ///         for byte in &mut uninitialized[..3] {
    ///             *byte = b'!'
    ///         }
    ///         3
    ///     })
    /// }
    /// assert_eq!(buf, b"hello!!!");
    /// ```
    pub unsafe fn write_to_uninitialized_tail<F>(&mut self, f: F)
    where F: FnOnce(&mut [u8]) -> usize {
        let (_, tail) = self.data_and_uninitialized_tail();
        let written = f(&mut *tail);
        let new_len = self.len().checked_add(written).expect("overflow");
        assert!(written <= (*tail).len());
        // Safety relies on the closure returning a correct value:
        self.set_len(new_len)
    }

    /// Extend this buffer by writing to its existing capacity.
    ///
    /// The closure is given a mutable bytes slice
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
    /// Panics if the value returned by the closure is larger than the given closure’s length.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// let mut buf = BytesBuf::from(b"hello".as_ref());
    /// buf.reserve(10);
    /// buf.write_to_zeroed_tail(|zeroed| {
    ///     for byte in &mut zeroed[..3] {
    ///         *byte = b'!'
    ///     }
    ///     10
    /// });
    /// assert_eq!(buf, b"hello!!!\0\0\0\0\0\0\0");
    /// ```
    pub fn write_to_zeroed_tail<F>(&mut self, f: F)
    where F: FnOnce(&mut [u8]) -> usize {
        unsafe {
            self.write_to_uninitialized_tail(|tail| {
                ptr::write_bytes(tail.as_mut_ptr(), 0, tail.len());
                f(tail)
            })
        }
    }

    /// Appends the given bytes slice onto the end of this buffer.
    ///
    /// This copies the existing data if this buffer is shared
    /// or if the existing capacity is insufficient.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// let mut buf = BytesBuf::from(b"hello".as_ref());
    /// buf.push_slice(b" world!");
    /// assert_eq!(buf, b"hello world!");
    /// ```
    pub fn push_slice(&mut self, slice: &[u8]) {
        self.reserve(slice.len());
        // Safety: copy_into_prefix’s contract
        unsafe {
            self.write_to_uninitialized_tail(|uninit| copy_into_prefix(slice, uninit))
        }
    }

    /// Appends the given bytes buffer onto the end of this buffer.
    ///
    /// This is similar to [`push_slice`](#method.push_slice), but sometimes more efficient.
    ///
    /// ## Examples
    ///
    /// This allocates only once:
    ///
    /// ```
    /// # use zbuf::BytesBuf;
    /// let string = "abc".repeat(20);
    /// let mut buf = BytesBuf::from(string.as_bytes());
    /// let tail = buf.split_off(50);
    /// assert_eq!(buf.len(), 50);
    /// assert_eq!(tail.len(), 10);
    /// buf.push_buf(&tail);
    /// assert_eq!(buf, string.as_bytes());
    /// ```
    pub fn push_buf(&mut self, other: &BytesBuf) {
        if self.is_empty() {
            *self = other.clone();
            return
        }

        // FIXME: remove when borrows are non-lexical
        fn raw<T>(x: &T) -> *const T { x }

        if let (Ok(a), Ok(b)) = (self.as_allocated().map(raw), other.as_allocated().map(raw)) {
            // Two heap-allocated buffers…
            if ptr::eq(a, b) {
                // … that share the same heap allocation…
                if (self.0.start + self.0.len) == other.0.start {
                    // … and are contiguous
                    self.0.len += other.0.len;
                    return
                }
            }
        }
        self.push_slice(other)
    }
}

/// Copy `source` entirely at the start of `dest`. Return the number of bytes copied.
#[inline]
unsafe fn copy_into_prefix(source: &[u8], dest: *mut [u8]) -> usize {
    let len = source.len();
    (*dest)[..len].copy_from_slice(source);
    len
}

impl Deref for BytesBuf {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self.as_allocated() {
            Ok(heap_allocation) => {
                let start = u32_to_usize(self.0.start);
                let len = u32_to_usize(self.0.len);
                // Safety: start..(start+len) is known to be initialized
                unsafe {
                    &(*heap_allocation.data())[start..][..len]
                }
            }
            Err(metadata) => {
                let len = inline_length(metadata);
                let struct_ptr: *const Inner = &self.0;
                let struct_ptr = struct_ptr as *const u8;
                // Safety relies on INLINE_DATA_OFFSET_BYTES being correct
                // and set_inline_length() checking that `len < INLINE_CAPACITY`,
                // which yields a slice within the memory layout of `Inner`.
                // Inline data is never uninitialized.
                unsafe {
                    let data_ptr = struct_ptr.offset(INLINE_DATA_OFFSET_BYTES);
                    slice::from_raw_parts(data_ptr, len)
                }
            }
        }
    }
}

/// This copies the existing data if there are other references to this buffer.
impl DerefMut for BytesBuf {
    fn deref_mut(&mut self) -> &mut [u8] {
        let (data, _) = self.data_and_uninitialized_tail();
        data
    }
}

impl AsRef<[u8]> for BytesBuf {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl AsMut<[u8]> for BytesBuf {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self
    }
}

impl<'a> From<&'a [u8]> for BytesBuf {
    #[inline]
    fn from(slice: &'a [u8]) -> Self {
        let mut buf = Self::new();
        buf.push_slice(slice);
        buf
    }
}

impl<'a, 'b> From<&'a &'b [u8]> for BytesBuf {
    #[inline]
    fn from(slice: &'a &'b [u8]) -> Self {
        let mut buf = Self::new();
        buf.push_slice(slice);
        buf
    }
}

impl fmt::Debug for BytesBuf {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        <[u8]>::fmt(self, formatter)
    }
}

impl hash::Hash for BytesBuf {
    #[inline]
    fn hash<H>(&self, hasher: &mut H) where H: hash::Hasher {
        <[u8]>::hash(self, hasher)
    }
}

impl Default for BytesBuf {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Eq for BytesBuf {}

impl<T: AsRef<[u8]>> PartialEq<T> for BytesBuf {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        <[u8]>::eq(self, other.as_ref())
    }
}

impl Ord for BytesBuf {
    #[inline]
    fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
        <[u8]>::cmp(self, &other)
    }
}

impl<T: AsRef<[u8]>> PartialOrd<T> for BytesBuf {
    #[inline]
    fn partial_cmp(&self, other: &T) -> Option<::std::cmp::Ordering> {
        <[u8]>::partial_cmp(self, other.as_ref())
    }
}

impl<'a> Extend<&'a [u8]> for BytesBuf {
    fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=&'a [u8]> {
        for item in iter {
            self.push_slice(item)
        }
    }
}

impl<'a> FromIterator<&'a [u8]> for BytesBuf {
    fn from_iter<I>(iter: I) -> Self where I: IntoIterator<Item=&'a [u8]> {
        let mut buf = Self::new();
        buf.extend(iter);
        buf
    }
}

impl<'a> Extend<&'a BytesBuf> for BytesBuf {
    fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=&'a BytesBuf> {
        for item in iter {
            self.push_buf(item)
        }
    }
}

impl<'a> FromIterator<&'a BytesBuf> for BytesBuf {
    fn from_iter<I>(iter: I) -> Self where I: IntoIterator<Item=&'a BytesBuf> {
        let mut buf = Self::new();
        buf.extend(iter);
        buf
    }
}

impl Extend<BytesBuf> for BytesBuf {
    fn extend<I>(&mut self, iter: I) where I: IntoIterator<Item=BytesBuf> {
        for item in iter {
            self.push_buf(&item)
        }
    }
}

impl FromIterator<BytesBuf> for BytesBuf {
    fn from_iter<I>(iter: I) -> Self where I: IntoIterator<Item=BytesBuf> {
        let mut buf = Self::new();
        buf.extend(iter);
        buf
    }
}
