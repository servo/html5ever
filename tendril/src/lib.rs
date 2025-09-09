// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//#![cfg_attr(test, deny(warnings))]
#![allow(clippy::result_unit_err)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::missing_transmute_annotations)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::wrong_self_convention)]
#![allow(clippy::transmute_bytes_to_str)]
#![allow(clippy::unusual_byte_groupings)]
#![allow(clippy::mutable_key_type)]

pub use fmt::Format;
pub use stream::TendrilSink;
pub use tendril::{Atomic, Atomicity, NonAtomic, SendTendril};
pub use tendril::{ByteTendril, ReadExt, SliceExt, StrTendril, SubtendrilError, Tendril};
pub use utf8_decode::IncompleteUtf8;

pub mod fmt;
pub mod stream;

mod buf32;
mod tendril;
mod utf8_decode;
mod util;

// Exposed for benchmarking purposes only
#[doc(hidden)]
pub mod futf;

static OFLOW: &str = "tendril: overflow in buffer arithmetic";
