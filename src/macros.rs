// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![macro_escape]

macro_rules! unwrap_or_return ( ($opt:expr, $retval:expr) => (
    match $opt {
        None => return $retval,
        Some(x) => x,
    }
));

macro_rules! test_eq ( ($name:ident, $left:expr, $right:expr) => (
    #[test]
    fn $name() {
        assert_eq!($left, $right);
    }
));

/// Make a tuple of the addresses of some of a struct's fields.
macro_rules! addrs_of ( ($obj:expr : $($field:ident),+) => (
    ( // make a tuple
        $(
            unsafe {
                ::core::mem::transmute::<_, uint>(&$obj.$field)
            }
        ),+
    )
));

// No format!() without libstd... just use the static message.
#[cfg(for_c)]
macro_rules! format_if ( ($pred:expr, $msg_static:expr, $msg_fmt:expr, $($arg:expr),*) => (
    ::std::borrow::Cow::Borrowed($msg_static)
));

#[cfg(not(for_c))]
macro_rules! format_if ( ($pred:expr, $msg_static:expr, $msg_fmt:expr, $($arg:expr),*) => (
    if $pred {
        ::std::borrow::Cow::Owned(format!($msg_fmt, $($arg),*))
    } else {
        ::std::borrow::Cow::Borrowed($msg_static)
    }
));

macro_rules! time ( ($e:expr) => ({
    let t0 = ::time::precise_time_ns();
    let result = $e;
    let dt = ::time::precise_time_ns() - t0;
    (result, dt)
}));

/// FIXME(rust-lang/rust#16806): copied from libcollections/macros.rs
#[cfg(for_c)]
macro_rules! vec(
    ($($e:expr),*) => ({
        // leading _ to allow empty construction without a warning.
        let mut _temp = ::collections::vec::Vec::new();
        $(_temp.push($e);)*
        _temp
    });
    ($($e:expr),+,) => (vec!($($e),+))
);

// Disable logging when building without the runtime.
#[cfg(for_c)]
mod log {
    #![macro_escape]
    macro_rules! h5e_log   (($($x:tt)*) => (()));
    macro_rules! h5e_debug (($($x:tt)*) => (()));
    macro_rules! h5e_info  (($($x:tt)*) => (()));
    macro_rules! h5e_warn  (($($x:tt)*) => (()));
    macro_rules! h5e_error (($($x:tt)*) => (()));
}

#[cfg(not(for_c))]
mod log {
    #![macro_escape]
    macro_rules! h5e_log   (($($x:tt)*) => (log!($($x)*)));
    macro_rules! h5e_debug (($($x:tt)*) => (debug!($($x)*)));
    macro_rules! h5e_info  (($($x:tt)*) => (info!($($x)*)));
    macro_rules! h5e_warn  (($($x:tt)*) => (warn!($($x)*)));
    macro_rules! h5e_error (($($x:tt)*) => (error!($($x)*)));
}
