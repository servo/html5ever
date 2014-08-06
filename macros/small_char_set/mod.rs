// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use rustc::plugin::Registry;

#[cfg(use_arch_byte_scan, target_arch="x86_64")]
#[path="x86_64.rs"]
pub mod arch;

#[cfg(use_arch_byte_scan)]
pub fn register(reg: &mut Registry) {
    reg.register_macro("small_char_set", arch::expand);
}

#[cfg(not(use_arch_byte_scan))]
pub fn register(_: &mut Registry) {
    // nothing
}
