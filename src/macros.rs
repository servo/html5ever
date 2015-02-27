// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

macro_rules! unwrap_or_else {
    ($opt:expr, $else_block:block) => {
        match $opt {
            None => $else_block,
            Some(x) => x,
        }
    }
}

macro_rules! unwrap_or_return {
    ($opt:expr, $retval:expr) => {
        unwrap_or_else!($opt, { return $retval })
    }
}

macro_rules! time {
    ($e:expr) => {{
        let t0 = ::time::precise_time_ns();
        let result = $e;
        let dt = ::time::precise_time_ns() - t0;
        (result, dt)
    }}
}

// Disable logging when building without the runtime.
#[cfg(for_c)]
#[macro_use]
mod log {
    macro_rules! h5e_log   (($($x:tt)*) => (()));
    macro_rules! h5e_debug (($($x:tt)*) => (()));
    macro_rules! h5e_info  (($($x:tt)*) => (()));
    macro_rules! h5e_warn  (($($x:tt)*) => (()));
    macro_rules! h5e_error (($($x:tt)*) => (()));
}

#[cfg(not(for_c))]
#[macro_use]
mod log {
    macro_rules! h5e_log   (($($x:tt)*) => (log!($($x)*)));
    macro_rules! h5e_debug (($($x:tt)*) => (debug!($($x)*)));
    macro_rules! h5e_info  (($($x:tt)*) => (info!($($x)*)));
    macro_rules! h5e_warn  (($($x:tt)*) => (warn!($($x)*)));
    macro_rules! h5e_error (($($x:tt)*) => (error!($($x)*)));
}
