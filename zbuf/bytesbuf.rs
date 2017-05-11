use heap_data::TaggedPtr;
use std::fmt;
use std::hash;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::slice;
use u32_to_usize;
use usize_to_u32;

/// A reference-counted bytes buffer.
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
    let with_new_len = without_len & (new_len << INLINE_LENGTH_OFFSET_BITS);
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
    pub fn new() -> Self {
        let metadata = 0;  // Includes bits for `length = 0`
        BytesBuf(Inner {
            ptr: TaggedPtr::new_inline_data(metadata),
            start: 0,
            len: 0,
        })
    }

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

    pub fn len(&self) -> usize {
        match self.0.ptr.as_allocated() {
            Ok(_) => u32_to_usize(self.0.len),
            Err(metadata) => inline_length(metadata),
        }
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
            unsafe {
                let data_ptr = (struct_ptr as *mut u8).offset(INLINE_DATA_OFFSET_BYTES);
                let inline = slice::from_raw_parts_mut(data_ptr, INLINE_CAPACITY);
                let (initialized, tail) = inline.split_at_mut(len);
                return (initialized, tail)
            }
        }

        let heap_data = self.0.ptr.as_owned_allocated_mut().expect("expected owned allocation");

        let start = u32_to_usize(self.0.start);
        let len = u32_to_usize(self.0.len);
        let data = heap_data.data_mut();
        unsafe {
            let (initialized, tail) = (*data)[start..].split_at_mut(len);
            return (initialized, tail)
        }
    }

    pub fn capacity(&self) -> usize {
        if let Ok(heap_data) = self.0.ptr.as_allocated() {
            let capacity = if heap_data.is_owned() {
                heap_data.data_capacity().checked_sub(self.0.start).expect("data_capacity < start ??")
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

    /// This does not copy any heap-allocated data.
    pub fn pop_front(&mut self, bytes: usize) {
        if let Ok(_) = self.0.ptr.as_allocated() {
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
        self.truncate(0)
    }

    /// This does not copy any data.
    pub fn truncate(&mut self, new_len: usize) {
        if new_len < self.len() {
            unsafe {
                self.set_len(new_len)
            }
        }
    }

    /// Unsafe: 0..new_len data must be initialized
    unsafe fn set_len(&mut self, new_len: usize) {
        match self.0.ptr.as_allocated() {
            Ok(_) => {
                self.0.len = usize_to_u32(new_len)
            }
            Err(metadata) => {
                self.0.ptr = TaggedPtr::new_inline_data(set_inline_length(metadata, new_len))
            }
        }
    }

    /// This copies the data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub fn reserve(&mut self, additional: usize) {
        let new_capacity = self.len().checked_add(additional).expect("overflow");
        // self.capacity() already caps at self.len() for shared (not owned) heap-allocated buffers.
        if new_capacity > self.capacity() {
            let mut copy = Self::with_capacity(new_capacity);
            copy.write_to_uninitialized_tail(|uninit| unsafe {
                copy_into_prefix(self, uninit)
            });
            *self = copy
        }
    }

    /// Unsafe: the closure must not *read* from the given slice, which may be uninitialized.
    ///
    /// The closure is given a raw mutable slice of potentially-uninitialized bytes,
    /// and returns the number of consecutive bytes written from the start of the slice.
    /// The buffer’s length is incremented by that much.
    ///
    /// If `self.reserve(additional)` is called immediately before this method,
    /// the slice is at least `additional` bytes long.
    /// Without a `reserve` call the slice can be any length, including zero.
    ///
    /// This copies the existing data if there are other references to this buffer.
    pub fn write_to_uninitialized_tail<F>(&mut self, f: F) where F: FnOnce(*mut [u8]) -> usize {
        let (_, tail) = self.data_and_uninitialized_tail();
        let written = f(tail);
        let new_len = self.len().checked_add(written).expect("overflow");
        unsafe {
            assert!(written <= (*tail).len());
            self.set_len(new_len)
        }
    }

    /// This copies the existing data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub fn push_slice(&mut self, slice: &[u8]) {
        self.reserve(slice.len());
        self.write_to_uninitialized_tail(|uninit| unsafe {
            copy_into_prefix(slice, uninit)
        })
    }
}

unsafe fn copy_into_prefix(source: &[u8], dest: *mut [u8]) -> usize {
    let len = source.len();
    (*dest)[..len].copy_from_slice(source);
    len
}

impl Deref for BytesBuf {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self.0.ptr.as_allocated() {
            Ok(heap_data) => {
                let start = u32_to_usize(self.0.start);
                let len = u32_to_usize(self.0.len);
                unsafe {
                    &(*heap_data.data())[start..][..len]
                }
            }
            Err(metadata) => {
                let len = inline_length(metadata);
                let struct_ptr: *const Inner = &self.0;
                let struct_ptr = struct_ptr as *const u8;
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
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl AsMut<[u8]> for BytesBuf {
    fn as_mut(&mut self) -> &mut [u8] {
        self
    }
}

impl<'a> From<&'a [u8]> for BytesBuf {
    fn from(slice: &'a [u8]) -> Self {
        let mut buf = Self::new();
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
