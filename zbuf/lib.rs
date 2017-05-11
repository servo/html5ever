/// FIXME: remove this module and use std::ptr::Shared instead once it is stable.
/// https://github.com/rust-lang/rust/issues/27730
mod shared_ptr;

mod heap_data;
mod bytesbuf;
mod strbuf;

pub use bytesbuf::BytesBuf;
pub use strbuf::StrBuf;

#[cfg(any(target_pointer_width = "32",
          target_pointer_width = "64"))]
pub fn u32_to_usize(x: u32) -> usize {
    x as usize  // Valid because usize is at least as big as u32
}

#[cfg(any(target_pointer_width = "32",
          target_pointer_width = "64"))]
pub fn usize_to_u32(x: usize) -> u32 {
    const MAX: usize = ::std::u32::MAX as usize;  // Valid because usize is at least as big as u32
    assert!(x <= MAX, "overflow");
    x as u32
}
