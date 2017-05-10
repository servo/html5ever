/// FIXME: remove this module and use std::ptr::Shared instead once it is stable.
/// https://github.com/rust-lang/rust/issues/27730
mod shared_ptr;

mod conversions;
mod heap_data;
mod bytesbuf;
mod strbuf;

pub use bytesbuf::BytesBuf;
pub use strbuf::StrBuf;
