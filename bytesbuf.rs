use conversions::{u32_to_usize, usize_to_u32};
use heap_data::HeapData;
use shared_ptr::Shared;
use std::fmt;
use std::hash;
use std::ops::{Deref, DerefMut, Range};

/// A reference-counted bytes buffer.
pub struct BytesBuf(Inner);

struct Inner {
    ptr: Shared<HeapData>,
    start: u32,
    len: u32,
}

impl BytesBuf {
    pub fn new() -> Self {
        Self::with_capacity(8)  // FIXME inline buf
    }

    pub fn with_capacity(capacity: usize) -> Self {
        BytesBuf(Inner {
            ptr: HeapData::allocate(usize_to_u32(capacity)),
            start: 0,
            len: 0,
        })
    }

    pub fn len(&self) -> usize {
        u32_to_usize(self.0.len)
    }

    fn heap_data(&self) -> &HeapData {
        unsafe {
            self.0.ptr.as_ref()
        }
    }

    /// Unsafe: may not be initialized
    unsafe fn data_after_start_make_mut(&mut self) -> &mut [u8] {
        if !self.heap_data().is_owned() {
            let copy = {
                let slice: &[u8] = self;
                Self::from(slice)
            };
            *self = copy
        }
        let data = self.0.ptr.as_mut().data_mut();

        // Slice here because call sites borrow `self` entirely
        // and therefore cannot access self.0.start after this call while the return slice
        // is in scope.
        // Accessing `self.0.start` before this call is incorrect
        // because this call can change it (reset it from non-zero to zero, when copying).
        // Accessing `self.0.len` before this call is fine, this method never changes the length.
        let start = u32_to_usize(self.0.start);
        &mut data[start..]
    }

    pub fn capacity(&self) -> usize {
        let heap_data = self.heap_data();
        let capacity = if heap_data.is_owned() {
            heap_data.data_capacity().checked_sub(self.0.start).expect("data_capacity < start ??")
        } else {
            self.0.len
        };
        u32_to_usize(capacity)
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.heap_data().data_ptr()
    }

    /// This does not copy any data.
    pub fn slice(&mut self, byte_range: Range<usize>) -> Self {
        let start = usize_to_u32(byte_range.start);
        let end = usize_to_u32(byte_range.end);
        match end.checked_sub(start) {
            None => panic!("slicing with a range {:?} that ends before it starts", byte_range),
            Some(len) => {
                if end > self.0.len {
                    panic!("slice out of range: {:?} with length = {}", byte_range, self.0.len)
                }
                self.heap_data().increment_refcount();
                BytesBuf(Inner {
                    ptr: self.0.ptr,
                    start: start,
                    len: len,
                })
            }
        }
    }

    /// This does not copy any data.
    pub fn pop_front(&mut self, bytes: usize) {
        let bytes = usize_to_u32(bytes);
        match self.0.len.checked_sub(bytes) {
            Some(new_len) => {
                self.0.start = self.0.start.checked_add(bytes).expect("overflow");
                self.0.len = new_len;
            }
            None => panic!("tried to pop {} bytes, only {} are available", bytes, self.0.len)
        }
    }

    /// This does not copy any data.
    pub fn pop_back(&mut self, bytes: usize) {
        match self.0.len.checked_sub(usize_to_u32(bytes)) {
            Some(new_len) => self.0.len = new_len,
            None => panic!("tried to pop {} bytes, only {} are available", bytes, self.0.len)
        }
    }

    /// This does not copy any data.
    pub fn truncate(&mut self, new_len: usize) {
        let new_len = usize_to_u32(new_len);
        if new_len < self.0.len {
            self.0.len = new_len
        }
    }

    /// This copies the data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub fn reserve(&mut self, additional: usize) {
        if additional == 0 {
            return  // No need to copy even if not owned
        }

        let new_capacity = self.len().checked_add(additional).expect("overflow");
        {
            let heap_data = self.heap_data();
            if heap_data.is_owned() && new_capacity <= self.capacity() {
                return
            }
        }

        let mut copy = Self::with_capacity(new_capacity);
        copy.push_slice(self);
        *self = copy
    }

    /// Unsafe: the closure must not *read* from the given slice, which may be uninitialized.
    ///
    /// The closure is given a mutable slice of at least `bytes_to_reserve` bytes,
    /// and returns the number of consecutive bytes written from the start of the slice.
    /// The buffer’s length is incremented by that much.
    ///
    /// This copies the existing data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub unsafe fn write_to_uninitialized<F>(&mut self, bytes_to_reserve: usize, f: F)
    where F: FnOnce(&mut [u8]) -> usize {
        self.reserve(bytes_to_reserve);
        let written;
        {
            let len = self.len();
            let data = self.data_after_start_make_mut();
            let uninitialized = &mut data[len..];
            written = f(uninitialized);
            assert!(written <= uninitialized.len());
        }
        self.0.len = self.0.len.checked_add(usize_to_u32(written)).expect("overflow");
    }

    /// This copies the existing data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub fn push_slice(&mut self, slice: &[u8]) {
        unsafe {
            let slice_len = slice.len();
            self.write_to_uninitialized(slice_len, |uninitialized| {
                uninitialized[..slice_len].copy_from_slice(slice);
                slice_len
            })
        }
    }
}

impl Drop for BytesBuf {
    fn drop(&mut self) {
        unsafe {
            HeapData::decrement_refcount_or_deallocate(self.0.ptr)
        }
    }
}

impl Deref for BytesBuf {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe {
            let data = self.heap_data().data();
            let start = u32_to_usize(self.0.start);
            let len = self.len();
            &data[start..][..len]
        }
    }
}

/// This copies the existing data if there are other references to this buffer.
impl DerefMut for BytesBuf {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe {
            let len = self.len();
            let data = self.data_after_start_make_mut();
            &mut data[..len]
        }
    }
}

impl AsRef<[u8]> for BytesBuf {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl AsMut<[u8]> for BytesBuf {
    fn as_mut(&mut self) -> &mut [u8] {
        self
    }
}

impl Clone for BytesBuf {
    fn clone(&self) -> Self {
        self.heap_data().increment_refcount();
        BytesBuf(Inner {
            ptr: self.0.ptr,
            start: self.0.start,
            len: self.0.len,
        })
    }
}

impl<'a> From<&'a [u8]> for BytesBuf {
    fn from(slice: &'a [u8]) -> Self {
        let mut buf = Self::with_capacity(slice.len());
        buf.push_slice(slice);
        buf
    }
}

impl fmt::Debug for BytesBuf {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        <[u8]>::fmt(self, formatter)
    }
}

impl hash::Hash for BytesBuf {
    fn hash<H>(&self, hasher: &mut H) where H: hash::Hasher {
        <[u8]>::hash(self, hasher)
    }
}

impl Default for BytesBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl Eq for BytesBuf {}

impl<T: AsRef<[u8]>> PartialEq<T> for BytesBuf {
    fn eq(&self, other: &T) -> bool {
        <[u8]>::eq(self, other.as_ref())
    }
}

impl Ord for BytesBuf {
    fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
        <[u8]>::cmp(self, &other)
    }
}

impl<T: AsRef<[u8]>> PartialOrd<T> for BytesBuf {
    fn partial_cmp(&self, other: &T) -> Option<::std::cmp::Ordering> {
        <[u8]>::partial_cmp(self, other.as_ref())
    }
}
