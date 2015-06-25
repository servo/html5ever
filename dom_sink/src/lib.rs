// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_name="html5ever_dom_sink"]
#![crate_type="dylib"]

#![feature(box_syntax, append, rc_weak)]

extern crate html5ever;

#[macro_use]
extern crate string_cache;

extern crate tendril;

#[macro_use]
extern crate mac;

pub mod common;
pub mod rcdom;
pub mod owned_dom;
