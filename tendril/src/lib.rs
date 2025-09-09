// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//#![cfg_attr(test, deny(warnings))]
#![allow(unnecessary_transmutes)]
#![allow(bare_trait_objects)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_late_init)]
#![allow(clippy::explicit_auto_deref)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::op_ref)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::missing_transmute_annotations)]
#![allow(clippy::partialeq_ne_impl)]
#![allow(clippy::legacy_numeric_constants)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::wrong_self_convention)]
#![allow(clippy::len_zero)]
#![allow(clippy::transmute_bytes_to_str)]
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::redundant_static_lifetimes)]
#![allow(clippy::redundant_field_names)]
#![allow(clippy::unusual_byte_groupings)]
#![allow(clippy::borrow_deref_ref)]
#![allow(clippy::needless_return)]
#![allow(clippy::while_let_loop)]
#![allow(clippy::mutable_key_type)]
#![allow(clippy::manual_repeat_n)]
#![allow(clippy::map_clone)]
#![allow(clippy::useless_conversion)]

#[macro_use]
extern crate debug_unreachable;
#[cfg(feature = "encoding")]
pub extern crate encoding;
#[cfg(feature = "encoding_rs")]
pub extern crate encoding_rs;
#[macro_use]
extern crate mac;
extern crate utf8;

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

static OFLOW: &'static str = "tendril: overflow in buffer arithmetic";
