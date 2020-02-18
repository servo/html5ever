//! [`BytesBuf`](struct.BytesBuf.html) and [`StrBuf`](struct.StrBuf.html)
//! are “zero-copy”<sup>[[1]](#zero-copy)</sup> buffers.
//! They are semantically equivalent to `Vec<u8>` and `String` respectively
//! (they derefence to `&[u8]` / `&str`, they are mutable and growable),
//! but have optimizations that minimize the number of data copies and memory allocations required.
//!
//! * **Inline buffers** (a.k.a. small string optimization):
//!   Small buffers (up to 15 bytes on 64-bit platforms, up to 11 bytes on 32-bit platforms)
//!   are stored inline and do not allocate heap memory.
//! * **Reference counting**:
//!   Multiple buffers can refer to the same reference-counted heap allocation.
//!   Cloning a buffer is cheap: it never allocates, at most it increments a reference count.
//! * **Slicing without borrowing**:
//!   A buffer can be a sub-slice of another without being tied to its lifetime,
//!   nor allocating more heap memory.
//!
//! Limitations:
//!
//! * **Up to 4 GB**:
//!   To keep the `std::mem::size_of()` of buffers more compact,
//!   sizes are stored as `u32` internally.
//!   Trying to allocate more than 4 gigabytes will panic,
//!   even on 64-bit platforms with enough RAM available.
//! * **No cheap conversions with standard library vectors**:
//!   In heap-allocated buffers the data is stored next to a header of metadata.
//!   Conversion to or from `Vec<u8>` / `Box<[u8]>` / `String` / `Box<str>`
//!   therefore necessarily goes through slices and incurs and data copy and memory allocation.
//!
//! ----
//!
//! <p id=zero-copy>[1] Disclaimer:
//! we use “zero-copy” with quotes because it is an exaggeration.
//! In typical usage there is at least one copy,
//! for example from the kernel’s network stack to a newly allocated buffer.
//! However, the library’s design is intended to minimize the number of copies needed after that.

extern crate utf8;

mod bytesbuf;
mod heap_data;
mod strbuf;
mod utf8_decoder;

pub use bytesbuf::BytesBuf;
pub use strbuf::{FromUtf8Error, StrBuf};
pub use utf8_decoder::{LossyUtf8Decoder, StrictUtf8Decoder, Utf8DecoderError};

#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
fn u32_to_usize(x: u32) -> usize {
    x as usize // Valid because usize is at least as big as u32
}

fn usize_to_u32(x: usize) -> u32 {
    std::convert::TryFrom::try_from(x).unwrap_or_else(|_| panic!("overflow"))
}
