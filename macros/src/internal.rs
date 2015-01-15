// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Macros for use in defining other macros.  Not exported.

#![macro_escape]

macro_rules! bail ( ($cx:expr, $sp:expr, $msg:expr) => ({
    $cx.span_err($sp, $msg);
    return ::syntax::ext::base::DummyResult::any($sp);
}));

macro_rules! bail_if ( ($e:expr, $cx:expr, $sp:expr, $msg:expr) => (
    if $e { bail!($cx, $sp, $msg) }
));

macro_rules! expect ( ($cx:expr, $sp:expr, $e:expr, $msg:expr) => (
    match $e {
        Some(x) => x,
        None => bail!($cx, $sp, $msg),
    }
));

