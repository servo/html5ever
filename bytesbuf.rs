use conversions::{u32_to_usize, usize_to_u32};
use heap_data::HeapData;
use shared_ptr::Shared;
use std::fmt;
use std::hash;
use std::ops::{Deref, DerefMut};

/// A reference-counted bytes buffer.
pub struct BytesBuf {
    ptr: Shared<HeapData>,
    len: u32,
}

impl BytesBuf {
    pub fn new() -> Self {
        Self::with_capacity(8)  // FIXME inline buf
    }

    pub fn with_capacity(capacity: usize) -> Self {
        BytesBuf {
            ptr: HeapData::allocate(usize_to_u32(capacity)),
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        u32_to_usize(self.len)
    }

    fn heap_data(&self) -> &HeapData {
        unsafe {
            self.ptr.as_ref()
        }
    }

    fn heap_data_make_mut(&mut self) -> &mut HeapData {
        if !self.heap_data().is_owned() {
            let copy = {
                let slice: &[u8] = self;
                Self::from(slice)
            };
            *self = copy
        }
        unsafe {
            self.ptr.as_mut()
        }
    }

    pub fn capacity(&self) -> usize {
        let heap_data = self.heap_data();
        let capacity = if heap_data.is_owned() {
            heap_data.data_capacity()
        } else {
            self.len
        };
        u32_to_usize(capacity)
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.heap_data().data_ptr()
    }

    pub fn pop_back(&mut self, bytes: usize) {
        match self.len.checked_sub(usize_to_u32(bytes)) {
            Some(new_len) => self.len = new_len,
            None => panic!("Tried to pop {} bytes, only {} are available", bytes, self.len)
        }
    }

    pub fn truncate(&mut self, new_len: usize) {
        let new_len = usize_to_u32(new_len);
        if new_len < self.len {
            self.len = new_len
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
            if heap_data.is_owned() && new_capacity <= u32_to_usize(heap_data.data_capacity()) {
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
            let len = u32_to_usize(self.len);
            let data = self.heap_data_make_mut().data_mut();
            let uninitialized = &mut data[len..];
            written = f(uninitialized);
            assert!(written <= uninitialized.len());
        }
        self.len += usize_to_u32(written);
    }

    /// This copies the existing data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub fn push_slice(&mut self, slice: &[u8]) {
        unsafe {
            self.write_to_uninitialized(slice.len(), |uninitialized| {
                uninitialized[..slice.len()].copy_from_slice(slice);
                slice.len()
            })
        }
    }
}

impl Drop for BytesBuf {
    fn drop(&mut self) {
        unsafe {
            HeapData::decrement_refcount_or_deallocate(self.ptr)
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

/// This copies the existing data if there are other references to this buffer.
impl DerefMut for BytesBuf {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe {
            let len = self.len();
            let data = self.heap_data_make_mut().data_mut();
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
        BytesBuf {
            ptr: self.ptr,
            len: self.len,
        }
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
