// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(quote, rustc_private)]
#![deny(warnings)]

extern crate syntax;

#[macro_use]
extern crate mac;

// See https://github.com/rust-lang/rust/pull/23857
macro_rules! panictry {
    ($e:expr) => ({
        use syntax::diagnostic::FatalError;
        match $e {
            Ok(e) => e,
            Err(FatalError) => panic!(FatalError)
        }
    })
}

// Make these public so that rustdoc will generate documentation for them.
pub mod match_token;
pub mod pre_expand;

pub use pre_expand::pre_expand;
