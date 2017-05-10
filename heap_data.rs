//! Heap-allocated data: a header followed by bytes

use conversions::u32_to_usize;
use shared_ptr::Shared;
use std::mem;
use std::slice;

#[repr(C)]  // Preserve field order: data is last
pub struct HeapData {
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
    pub fn data_capacity_to_vec_capacity(data_bytes: u32) -> usize {
        let header_size = mem::size_of::<HeapData>();
        let bytes = u32_to_usize(data_bytes).checked_add(header_size).expect("overflow");
        // Integer ceil http://stackoverflow.com/a/2745086/1162888
        1 + (bytes - 1).checked_div(header_size).expect("zero-size header?")
    }

    pub fn allocate(data_capacity: u32) -> Shared<Self> {
        let vec_capacity = HeapData::data_capacity_to_vec_capacity(data_capacity);
        let mut vec = Vec::<HeapData>::with_capacity(vec_capacity);
        vec.push(HeapData {
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

    /// Unsafe: `ptr` must be valid, 0..len must be initialized
    pub unsafe fn reallocate(ptr: &mut Shared<Self>, len: u32, new_data_capacity: u32) {
        if new_data_capacity > ptr.as_ref().data_capacity {
            let mut new_ptr = HeapData::allocate(new_data_capacity);
            {
                let initialized = &ptr.as_ref().data()[..u32_to_usize(len)];
                let uninitialized = new_ptr.as_mut().data_mut();
                uninitialized[..initialized.len()].copy_from_slice(initialized);
            }
            let old_ptr = *ptr;
            *ptr = new_ptr;
            HeapData::deallocate(old_ptr)
        }
    }

    /// Unsafe: `ptr` must be valid, and not used afterwards
    pub unsafe fn deallocate(ptr: Shared<Self>) {
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

    pub fn data_capacity(&self) -> u32 {
        self.data_capacity
    }

    pub fn data_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }

    pub fn data_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }

    /// Unsafe: may not be initialized
    pub unsafe fn data(&self) -> &[u8] {
        slice::from_raw_parts(self.data_ptr(), u32_to_usize(self.data_capacity()))
    }

    /// Unsafe: may not be initialized
    pub unsafe fn data_mut(&mut self) -> &mut [u8] {
        slice::from_raw_parts_mut(self.data_mut_ptr(), u32_to_usize(self.data_capacity()))
    }
}
