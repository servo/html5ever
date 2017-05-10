use conversions::{u32_to_usize, usize_to_u32};
use heap_data::HeapData;
use shared_ptr::Shared;
use std::fmt;
use std::hash;
use std::ops::{Deref, DerefMut};

/// A bytes buffer.
///
/// Always owned, for now.
pub struct BytesBuf {
    ptr: Shared<HeapData>,
    bytes_len: u32,
}

impl BytesBuf {
    pub fn new() -> Self {
        Self::with_capacity(8)  // FIXME inline buf
    }

    pub fn with_capacity(capacity: usize) -> Self {
        BytesBuf {
            ptr: HeapData::allocate(usize_to_u32(capacity)),
            bytes_len: 0,
        }
    }

    pub fn len(&self) -> usize {
        u32_to_usize(self.bytes_len)
    }

    fn heap_data(&self) -> &HeapData {
        unsafe {
            self.ptr.as_ref()
        }
    }

    fn heap_data_mut(&mut self) -> &mut HeapData {
        unsafe {
            self.ptr.as_mut()
        }
    }

    pub fn capacity(&self) -> usize {
        u32_to_usize(self.heap_data().data_capacity())
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.heap_data().data_ptr()
    }

    pub fn truncate(&mut self, new_len: usize) {
        let new_len = usize_to_u32(new_len);
        if new_len < self.bytes_len {
            self.bytes_len = new_len
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        let len = self.bytes_len;
        let new_capacity = len.checked_add(usize_to_u32(additional)).expect("overflow");
        unsafe {
            HeapData::reallocate(&mut self.ptr, len, new_capacity)
        }
    }

    /// Unsafe: the closure must not *read* from the given slice, which is uninitialized.
    ///
    /// The closure return the number of consecutive bytes written from the start of the slice.
    /// The buffer’s length is incremented by that much.
    pub unsafe fn write_to_uninitialized<F>(&mut self, f: F) where F: FnOnce(&mut [u8]) -> usize {
        let written;
        {
            let len = u32_to_usize(self.bytes_len);
            let data = self.heap_data_mut().data_mut();
            let uninitialized = &mut data[len..];
            written = f(uninitialized);
            assert!(written <= uninitialized.len());
        }
        self.bytes_len += usize_to_u32(written);
    }

    pub fn push_slice(&mut self, slice: &[u8]) {
        self.reserve(slice.len());
        unsafe {
            self.write_to_uninitialized(|uninitialized| {
                uninitialized[..slice.len()].copy_from_slice(slice);
                slice.len()
            })
        }
    }
}

impl Drop for BytesBuf {
    fn drop(&mut self) {
        unsafe {
            HeapData::deallocate(self.ptr)
        }
    }
}

impl Deref for BytesBuf {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe {
            let len = self.len();
            let data = self.heap_data().data();
            &data[..len]
        }
    }
}

impl DerefMut for BytesBuf {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe {
            let len = self.len();
            let data = self.heap_data_mut().data_mut();
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
        From::<&[u8]>::from(self)
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

impl PartialEq for BytesBuf {
    fn eq(&self, other: &Self) -> bool {
        <[u8]>::eq(self, &**other)
    }
}

impl Ord for BytesBuf {
    fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
        <[u8]>::cmp(self, &other)
    }
}

impl PartialOrd for BytesBuf {
    fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
        <[u8]>::partial_cmp(self, &other)
    }
}
