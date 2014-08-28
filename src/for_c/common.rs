// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::prelude::*;

use core::slice::raw::buf_as_slice;
use core::str::raw::from_utf8;
use core::kinds::marker::ContravariantLifetime;

use libc::{size_t, c_int, c_char, strlen};

#[repr(C)]
pub struct h5e_buf {
    data: *const u8,
    len: size_t,
}

impl h5e_buf {
    pub fn null() -> h5e_buf {
        h5e_buf {
            data: RawPtr::null(),
            len: 0,
        }
    }

    pub unsafe fn with_slice<R>(&self, f: |&str| -> R) -> R {
        buf_as_slice(self.data, self.len as uint,
            |bytes| f(from_utf8(bytes)))
    }
}

pub struct LifetimeBuf<'a> {
    buf: h5e_buf,
    marker: ContravariantLifetime<'a>,
}

impl<'a> LifetimeBuf<'a> {
    pub fn from_str<T: Str>(x: &'a T) -> LifetimeBuf<'a> {
        let x = x.as_slice();
        LifetimeBuf {
            buf: h5e_buf {
                data: x.as_bytes().as_ptr(),
                len: x.len() as size_t,
            },
            marker: ContravariantLifetime,
        }
    }

    pub fn null() -> LifetimeBuf<'a> {
        LifetimeBuf {
            buf: h5e_buf::null(),
            marker: ContravariantLifetime,
        }
    }

    #[inline]
    pub fn get(self) -> h5e_buf {
        self.buf
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
