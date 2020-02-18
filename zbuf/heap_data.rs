//! Heap-allocated data: a header followed by bytes

use std::alloc::{self, Layout};
use std::cell::Cell;
use std::mem;
use std::ptr::NonNull;
use std::slice;
use u32_to_usize;
use usize_to_u32;

const TAG: usize = 1;
const TAG_MASK: usize = 1;

pub struct TaggedPtr(NonNull<HeapAllocation>);

impl TaggedPtr {
    pub fn allocate(requested_data_capacity: usize) -> Self {
        let ptr = HeapAllocation::allocate(requested_data_capacity);
        assert!(((ptr.as_ptr() as usize) & TAG_MASK) == 0);
        TaggedPtr(ptr)
    }

    #[inline]
    pub fn new_inline_data(data: usize) -> Self {
        let fake_ptr = (data | TAG) as *mut HeapAllocation;
        // Safety: TAG being non-zero makes `fake_ptr` non-null.
        unsafe { TaggedPtr(NonNull::new_unchecked(fake_ptr)) }
    }

    #[inline]
    fn as_valid_ptr(&self) -> Result<&NonNull<HeapAllocation>, usize> {
        let as_usize = self.0.as_ptr() as usize;
        if (as_usize & TAG_MASK) == 0 {
            Ok(&self.0)
        } else {
            Err(as_usize)
        }
    }

    #[inline]
    pub fn get_inline_data(&self) -> Result<usize, ()> {
        self.as_valid_ptr().err().ok_or(())
    }

    #[inline]
    pub fn as_allocated(&self) -> Result<&HeapAllocation, usize> {
        // Safety relies on `as_valid_ptr`, reference counting, and ownership of `TaggedPtr`.
        self.as_valid_ptr().map(|ptr| unsafe { ptr.as_ref() })
    }

    #[inline]
    pub fn as_owned_allocated_mut(&mut self) -> Option<&mut HeapAllocation> {
        match self.as_valid_ptr() {
            Err(_) => None,
            // Safety relies on `as_valid_ptr`, reference counting, and ownership of `TaggedPtr`.
            Ok(_) => unsafe {
                if self.0.as_ref().is_owned() {
                    Some(self.0.as_mut())
                } else {
                    None
                }
            },
        }
    }

    #[inline]
    pub fn is_shared_allocation(&self) -> bool {
        match self.as_valid_ptr() {
            Err(_) => false,
            // Safety relies on `as_valid_ptr`, reference counting, and ownership of `TaggedPtr`.
            Ok(ptr) => unsafe { !ptr.as_ref().is_owned() },
        }
    }
}

impl Clone for TaggedPtr {
    #[inline]
    fn clone(&self) -> Self {
        if let Ok(heap_allocation) = self.as_allocated() {
            heap_allocation.increment_refcount()
        }
        TaggedPtr(self.0)
    }
}

impl Drop for TaggedPtr {
    #[inline]
    fn drop(&mut self) {
        if let Ok(heap_allocation) = self.as_allocated() {
            let new_refcount = heap_allocation.decrement_refcount();
            if new_refcount == 0 {
                // Safety: we’re dropping the last reference
                unsafe { HeapAllocation::deallocate(self.0, heap_allocation.data_capacity) }
            }
        }
    }
}

#[repr(C)] // Preserve field order: data is last
pub struct HeapAllocation {
    refcount: Cell<u32>,
    data_capacity: u32,
    data: [u8; 0], // Actually dynamically-sized
}

impl HeapAllocation {
    fn allocate(requested_data_capacity: usize) -> NonNull<Self> {
        let header_size = mem::size_of::<HeapAllocation>();

        // We allocate space for one header, followed immediately by the data.
        let bytes = header_size
            .checked_add(requested_data_capacity)
            .expect("overflow");

        // Grow exponentially to amortize allocation/copying cost
        let bytes = bytes.checked_next_power_of_two().unwrap_or(bytes);

        let actual_data_capacity = usize_to_u32(bytes - header_size);

        unsafe {
            let layout = Layout::from_size_align(bytes, mem::align_of::<Self>()).unwrap();
            let mut ptr = NonNull::new(alloc::alloc(layout))
                .unwrap_or_else(|| alloc::handle_alloc_error(layout))
                .cast();
            *ptr.as_mut() = HeapAllocation {
                refcount: Cell::new(1),
                data_capacity: actual_data_capacity,
                data: [],
            };
            ptr
        }
    }

    #[inline]
    fn increment_refcount(&self) {
        self.refcount.set(
            self.refcount
                .get()
                .checked_add(1)
                .expect("refcount overflow"),
        )
    }

    #[inline]
    fn decrement_refcount(&self) -> u32 {
        let new_count = self
            .refcount
            .get()
            .checked_sub(1)
            .expect("refcount underflow");
        self.refcount.set(new_count);
        new_count
    }

    /// Unsafe: `ptr` must be valid, and not used afterwards
    #[inline(never)]
    #[cold]
    unsafe fn deallocate(ptr: NonNull<HeapAllocation>, data_capacity: u32) {
        let header_size = mem::size_of::<HeapAllocation>();
        let bytes = header_size + u32_to_usize(data_capacity);
        let layout = Layout::from_size_align(bytes, mem::align_of::<Self>()).unwrap();
        alloc::dealloc(ptr.cast().as_ptr(), layout)
    }

    #[inline]
    pub fn is_owned(&self) -> bool {
        self.refcount.get() == 1
    }

    #[inline]
    pub fn data_capacity(&self) -> u32 {
        self.data_capacity
    }

    #[inline]
    pub fn data(&self) -> *const [u8] {
        // Safety relies on `vec_capacity` in HeapAllocation::allocate being large enough.
        unsafe { slice::from_raw_parts(self.data.as_ptr(), u32_to_usize(self.data_capacity)) }
    }

    #[inline]
    pub fn data_mut(&mut self) -> *mut [u8] {
        // Safety relies on `vec_capacity` in HeapAllocation::allocate being large enough.
        unsafe {
            slice::from_raw_parts_mut(self.data.as_mut_ptr(), u32_to_usize(self.data_capacity))
        }
    }
}
