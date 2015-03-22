// Copyright 2014-2015 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate libc;
extern crate string_cache;
extern crate tendril;
extern crate html5ever;

use std::{ptr, slice, str};
use std::marker::PhantomData;
use std::borrow::Cow;

use libc::{size_t, c_int, c_char, strlen};

use string_cache::Atom;

use tendril::StrTendril;

#[repr(C)]
pub struct h5e_buf {
    data: *const u8,
    len: size_t,
}

impl Copy for h5e_buf { }
impl Clone for h5e_buf {
    fn clone(&self) -> h5e_buf {
        *self
    }
}

impl h5e_buf {
    pub fn null() -> h5e_buf {
        h5e_buf {
            data: ptr::null(),
            len: 0,
        }
    }

    pub unsafe fn as_slice(&self) -> &str {
        str::from_utf8_unchecked(slice::from_raw_parts(self.data, self.len as usize))
    }
}

pub struct LifetimeBuf<'a> {
    buf: h5e_buf,
    marker: PhantomData<&'a [u8]>,
}

impl<'a> LifetimeBuf<'a> {
    pub fn from_str(x: &'a str) -> LifetimeBuf<'a> {
        LifetimeBuf {
            buf: h5e_buf {
                data: x.as_bytes().as_ptr(),
                len: x.len() as size_t,
            },
            marker: PhantomData,
        }
    }

    pub fn null() -> LifetimeBuf<'a> {
        LifetimeBuf {
            buf: h5e_buf::null(),
            marker: PhantomData,
        }
    }

    #[inline]
    pub fn get(self) -> h5e_buf {
        self.buf
    }
}

// Or we could just make `LifetimeBuf::from_str` generic over <T: Str>;
// see rust-lang/rust#16738.
pub trait AsLifetimeBuf {
    fn as_lifetime_buf<'a>(&'a self) -> LifetimeBuf<'a>;
}

impl AsLifetimeBuf for String {
    fn as_lifetime_buf<'a>(&'a self) -> LifetimeBuf<'a> {
        LifetimeBuf::from_str(self)
    }
}

impl AsLifetimeBuf for StrTendril {
    fn as_lifetime_buf<'a>(&'a self) -> LifetimeBuf<'a> {
        LifetimeBuf::from_str(self)
    }
}

impl AsLifetimeBuf for Atom {
    fn as_lifetime_buf<'a>(&'a self) -> LifetimeBuf<'a> {
        LifetimeBuf::from_str(self)
    }
}

impl<'b> AsLifetimeBuf for Cow<'b, str> {
    fn as_lifetime_buf<'a>(&'a self) -> LifetimeBuf<'a> {
        LifetimeBuf::from_str(self)
    }
}

#[no_mangle]
pub unsafe extern "C" fn h5e_buf_from_cstr(s: *const c_char) -> h5e_buf {
    h5e_buf {
        data: s as *const u8,
        len: strlen(s),
    }
}

pub fn c_bool(x: bool) -> c_int {
    match x {
        false => 0,
        true => 1,
    }
}
