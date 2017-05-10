use conversions::{u32_to_usize, usize_to_u32};
use heap_data::HeapData;
use shared_ptr::Shared;
use std::fmt;
use std::hash;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::slice;

/// A reference-counted bytes buffer.
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
#[repr(C)]  // Don’t re-order fields
struct Inner {
    ptr: Shared<HeapData>,
    start: u32,
    len: u32,
}

#[cfg(target_endian = "big")]
#[repr(C)]  // Don’t re-order fields
struct Inner {
    start: u32,
    len: u32,
    ptr: Shared<HeapData>,
}

/// Offset from the start of `Inner` to the start of inline buffer data.
/// On little-endian the metadata byte is at the start, so inline data starts after that.
/// On big-endian the metadata byte is at the end of `Inner`.
#[cfg(target_endian = "little")]
const INLINE_DATA_OFFSET_BYTES: isize = 1;

#[cfg(target_endian = "big")]
const INLINE_DATA_OFFSET_BYTES: isize = 0;

const INLINE_TAG: usize = 1;
const TAG_MASK: usize = 0b_11;
const INLINE_LENGTH_MASK: usize = 0b_1111_1100;
const INLINE_LENGTH_OFFSET_BITS: usize = 2;

fn is_heap_allocated(ptr: Shared<HeapData>) -> bool {
    ((ptr.as_ptr() as usize) & TAG_MASK) == 0
}

fn inline_length(ptr: Shared<HeapData>) -> usize {
    ((ptr.as_ptr() as usize) & INLINE_LENGTH_MASK) >> INLINE_LENGTH_OFFSET_BITS
}

fn set_inline_length(ptr: &mut Shared<HeapData>, new_len: usize) {
    debug_assert!(new_len <= INLINE_CAPACITY);
    let without_len = (ptr.as_ptr() as usize) & !INLINE_LENGTH_MASK;
    let with_new_len = without_len & (new_len << INLINE_LENGTH_OFFSET_BITS);
    debug_assert!((with_new_len & TAG_MASK) != 0);
    unsafe {
        *ptr = Shared::new(with_new_len as *mut HeapData)
    }
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
        let ptr = INLINE_TAG;  // Length bits are zero
        BytesBuf(Inner {
            ptr: unsafe { Shared::new(ptr as *mut HeapData) },
            start: 0,
            len: 0,
        })
    }

    pub fn with_capacity(capacity: usize) -> Self {
        if capacity <= INLINE_CAPACITY {
            Self::new()
        } else {
            let ptr = HeapData::allocate(usize_to_u32(capacity));
            assert!(is_heap_allocated(ptr));
            BytesBuf(Inner {
                ptr: ptr,
                start: 0,
                len: 0,
            })
        }
    }

    pub fn len(&self) -> usize {
        if is_heap_allocated(self.0.ptr) {
            u32_to_usize(self.0.len)
        } else {
            inline_length(self.0.ptr)
        }
    }

    fn heap_data(&self) -> Option<&HeapData> {
        if is_heap_allocated(self.0.ptr) {
            unsafe {
                Some(self.0.ptr.as_ref())
            }
        } else {
            None
        }
    }

    /// Unsafe: may not be initialized
    unsafe fn data_after_start_make_mut(&mut self) -> &mut [u8] {
        // FIXME: use `if let Some(heap_data) = self.heap_data() {` when borrows are non-lexical.
        if is_heap_allocated(self.0.ptr) {
            if !self.0.ptr.as_ref().is_owned() {
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
        } else {
            let struct_ptr: *mut Inner = &mut self.0;
            let data_ptr = (struct_ptr as *mut u8).offset(INLINE_DATA_OFFSET_BYTES);
            slice::from_raw_parts_mut(data_ptr, INLINE_CAPACITY)
        }
    }

    pub fn capacity(&self) -> usize {
        if let Some(heap_data) = self.heap_data() {
            let capacity = if heap_data.is_owned() {
                heap_data.data_capacity().checked_sub(self.0.start).expect("data_capacity < start ??")
            } else {
                self.0.len
            };
            u32_to_usize(capacity)
        } else {
            INLINE_CAPACITY
        }
    }

    /// This does not copy any heap-allocated data.
    pub fn pop_front(&mut self, bytes: usize) {
        if is_heap_allocated(self.0.ptr) {
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

    /// Unsafe: `new_len <= self.len()` must hold
    unsafe fn set_len(&mut self, new_len: usize) {
        if is_heap_allocated(self.0.ptr) {
            self.0.len = usize_to_u32(new_len)
        } else {
            set_inline_length(&mut self.0.ptr, new_len)
        }
    }

    /// This copies the data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub fn reserve(&mut self, additional: usize) {
        let new_capacity = self.len().checked_add(additional).expect("overflow");
        // self.capacity() already caps at self.len() for shared (not owned) heap-allocated buffers.
        if new_capacity > self.capacity() {
            let mut copy = Self::with_capacity(new_capacity);
            unsafe {
                copy.write_to_uninitialized(|uninit| copy_into_prefix(self, uninit))
            }
            *self = copy
        }
    }

    /// Unsafe: the closure must not *read* from the given slice, which may be uninitialized.
    ///
    /// The closure is given a mutable slice of at least `bytes_to_reserve` bytes,
    /// and returns the number of consecutive bytes written from the start of the slice.
    /// The buffer’s length is incremented by that much.
    ///
    /// This copies the existing data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub unsafe fn write_to_uninitialized<F>(&mut self, f: F) where F: FnOnce(&mut [u8]) -> usize {
        let written;
        {
            let len = self.len();
            let data = self.data_after_start_make_mut();
            let uninitialized = &mut data[len..];
            written = f(uninitialized);
            assert!(written <= uninitialized.len());
        }
        let new_len = self.len().checked_add(written).expect("overflow");
        self.set_len(new_len)
    }

    /// This copies the existing data if there are other references to this buffer
    /// or if the existing capacity is insufficient.
    pub fn push_slice(&mut self, slice: &[u8]) {
        unsafe {
            self.reserve(slice.len());
            self.write_to_uninitialized(|uninit| copy_into_prefix(slice, uninit))
        }
    }
}

fn copy_into_prefix(source: &[u8], dest: &mut [u8]) -> usize {
    let len = source.len();
    dest[..len].copy_from_slice(source);
    len
}

impl Drop for BytesBuf {
    fn drop(&mut self) {
        if is_heap_allocated(self.0.ptr) {
            unsafe {
                HeapData::decrement_refcount_or_deallocate(self.0.ptr)
            }
        }
    }
}

impl Deref for BytesBuf {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe {
            if let Some(heap_data) = self.heap_data() {
                let start = u32_to_usize(self.0.start);
                let len = u32_to_usize(self.0.len);
                &heap_data.data()[start..][..len]
            } else {
                let struct_ptr: *const Inner = &self.0;
                let data_ptr = (struct_ptr as *const u8).offset(INLINE_DATA_OFFSET_BYTES);
                slice::from_raw_parts(data_ptr, inline_length(self.0.ptr))
            }
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
        if let Some(heap_data) = self.heap_data() {
            heap_data.increment_refcount()
        }
        BytesBuf(Inner { ..self.0 })
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
