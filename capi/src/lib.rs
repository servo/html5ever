// Copyright 2014-2015 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(plugin, box_syntax)]
#![plugin(string_cache_plugin)]

extern crate libc;
extern crate string_cache;
extern crate tendril;
extern crate html5ever;

use libc::c_int;

pub mod tokenizer;

fn c_bool(x: bool) -> c_int {
    match x {
        false => 0,
        true => 1,
    }
}
