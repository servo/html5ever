//! Heap-allocated data: a header followed by bytes

use conversions::u32_to_usize;
use shared_ptr::Shared;
use std::cell::Cell;
use std::mem;
use std::slice;

#[repr(C)]  // Preserve field order: data is last
pub struct HeapData {
    refcount: Cell<u32>,
    data_capacity: u32,
    data: [u8; 0],  // Actually dynamically-sized
}

impl HeapData {
    /// We’re using Vec<HeapData> as a memory allocator.
    /// Return the capacity of that vector required to allocate a buffer
    /// that holds one header followed by at least the given number of bytes.
    ///
    /// Note: we’re not using Vec<HeapData> instead of Vec<u8>
    /// in order to request a memory alignment sufficient for the header.
    fn data_capacity_to_vec_capacity(data_bytes: u32) -> usize {
        let header_size = mem::size_of::<HeapData>();
        let bytes = u32_to_usize(data_bytes).checked_add(header_size).expect("overflow");
        // Integer ceil http://stackoverflow.com/a/2745086/1162888
        1 + (bytes - 1).checked_div(header_size).expect("zero-size header?")
    }

    pub fn allocate(data_capacity: u32) -> Shared<Self> {
        let vec_capacity = HeapData::data_capacity_to_vec_capacity(data_capacity);
        let mut vec = Vec::<HeapData>::with_capacity(vec_capacity);
        vec.push(HeapData {
            refcount: Cell::new(1),
            data_capacity: data_capacity,
            data: [],
        });
        debug_assert_eq!(vec.capacity(), vec_capacity);
        let ptr = vec.as_mut_ptr();
        mem::forget(vec);
        unsafe {
            Shared::new(ptr)
        }
    }

    pub fn increment_refcount(&self) {
        self.refcount.set(self.refcount.get().checked_add(1).expect("refcount overflow"))
    }

    /// Unsafe: `ptr` must be valid, and not used afterwards
    pub unsafe fn decrement_refcount_or_deallocate(ptr: Shared<HeapData>) {
        let count = ptr.as_ref().refcount.get();
        if count > 1 {
            ptr.as_ref().refcount.set(count - 1);
        } else {
            // Deallocate

            // `ptr` points to a memory area `size_of::<HeapData> * vec_capacity` bytes wide
            // that starts with one `HeapData` header and is followed by data bytes.
            // `length == 1` is the correct way to represent this in terms of `Vec`,
            // even though in practice `length` doesn’t make a difference
            // since `HeapData` does not have a destructor.
            let vec_length = 1;
            let vec_capacity = HeapData::data_capacity_to_vec_capacity(ptr.as_ref().data_capacity);

            let vec = Vec::<HeapData>::from_raw_parts(ptr.as_ptr(), vec_length, vec_capacity);
            mem::drop(vec);
        }
    }

    pub fn is_owned(&self) -> bool {
        self.refcount.get() == 1
    }

    pub fn data_capacity(&self) -> u32 {
        self.data_capacity
    }

    pub fn data_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }

    /// Unsafe: may not be initialized
    pub unsafe fn data(&self) -> &[u8] {
        slice::from_raw_parts(self.data_ptr(), u32_to_usize(self.data_capacity()))
    }

    /// Unsafe: may not be initialized
    pub unsafe fn data_mut(&mut self) -> &mut [u8] {
        slice::from_raw_parts_mut(self.data.as_mut_ptr(), u32_to_usize(self.data_capacity()))
    }
}
