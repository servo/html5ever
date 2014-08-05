// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Exported non-procedural macros.

#[macro_export]
macro_rules! unwrap_or_return ( ($opt:expr, $retval:expr) => (
    match $opt {
        None => return $retval,
        Some(x) => x,
    }
))

#[macro_export]
macro_rules! test_eq ( ($name:ident, $left:expr, $right:expr) => (
    #[test]
    fn $name() {
        assert_eq!($left, $right);
    }
))

/// Make a tuple of the addresses of some of a struct's fields.
#[macro_export]
macro_rules! addrs_of ( ($obj:expr : $($field:ident),+) => (
    ( // make a tuple
        $(
            unsafe {
                ::std::mem::transmute::<_, uint>(&$obj.$field)
            }
        ),+
    )
))

#[macro_export]
macro_rules! format_if ( ($pred:expr, $msg_static:expr, $msg_fmt:expr, $($arg:expr),*) => (
    if $pred {
        ::std::str::Owned(format!($msg_fmt, $($arg),*))
    } else {
        ::std::str::Slice($msg_static)
    }
))

