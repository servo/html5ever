//! Heap-allocated data: a header followed by bytes

use shared_ptr::Shared;
use std::cell::Cell;
use std::mem;
use std::slice;
use u32_to_usize;
use usize_to_u32;

#[repr(C)]  // Preserve field order: data is last
pub struct HeapData {
    refcount: Cell<u32>,
    data_capacity: u32,
    data: [u8; 0],  // Actually dynamically-sized
}

impl HeapData {
    pub fn allocate(requested_data_capacity: usize) -> Shared<Self> {
        let header_size = mem::size_of::<HeapData>();

        // We allocate space for one header, followed immediately by the data.
        let bytes = header_size.checked_add(requested_data_capacity).expect("overflow");

        // Grow exponentially to amortize allocation/copying cost
        let bytes = bytes.checked_next_power_of_two().unwrap_or(bytes);

        let actual_data_capacity = usize_to_u32(bytes - header_size);

        // alloc::heap::allocate is unstable (https://github.com/rust-lang/rust/issues/27700),
        // so we use `Vec` as a memory allocator.
        // To get correct memory alignment for the header,
        // we use `Vec<HeapData>` rather than `Vec<u8>`.
        // So the vector’s capacity is counted in “number of `HeapData` items”, not bytes.
        //
        // Integer division rounding up: http://stackoverflow.com/a/2745086/1162888
        let vec_capacity = 1 + ((bytes - 1) / header_size);

        let mut vec = Vec::<HeapData>::with_capacity(vec_capacity);
        debug_assert_eq!(vec.capacity(), vec_capacity);
        vec.push(HeapData {
            refcount: Cell::new(1),
            data_capacity: actual_data_capacity,
            data: [],
        });
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
        let as_ref = ptr.as_ref();
        let count = as_ref.refcount.get();
        if count > 1 {
            as_ref.refcount.set(count - 1);
        } else {
            // Deallocate

            let header_size = mem::size_of::<HeapData>();
            let allocated_bytes = header_size + u32_to_usize(as_ref.data_capacity);
            let vec_capacity = allocated_bytes / header_size;

            // `ptr` points to a memory area `size_of::<HeapData> * vec_capacity` bytes wide
            // that starts with one `HeapData` header and is followed by data bytes.
            // `length == 1` is the correct way to represent this in terms of `Vec`,
            // even though in practice `length` doesn’t make a difference
            // since `HeapData` does not have a destructor.
            let vec_length = 1;

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
