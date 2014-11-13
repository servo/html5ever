// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::prelude::*;

use core::slice;
use core::str;
use core::raw::Repr;
use core::kinds::marker::ContravariantLifetime;
use collections::str::MaybeOwned;
use collections::vec::Vec;

use libc::{size_t, c_int, c_char, strlen};

use string_cache::Atom;

use util::span::Span;

use iobuf::Iobuf;

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

    pub unsafe fn as_slice(&self) -> &str {
        str::from_utf8_unchecked(slice::from_raw_buf(&self.data, self.len as uint))
    }
}

/// A set of h5e buffers.
#[repr(C)]
pub struct h5e_bufset {
    data: *const h5e_buf,
    len:  size_t,
}

impl h5e_bufset {
    pub fn null() -> h5e_bufset {
        h5e_bufset {
            data: RawPtr::null(),
            len: 0,
        }
    }
}

pub struct LifetimeBuf<'a> {
    buf: h5e_buf,
    marker: ContravariantLifetime<'a>,
}

impl<'a> LifetimeBuf<'a> {
    pub fn from_str(x: &'a str) -> LifetimeBuf<'a> {
        LifetimeBuf {
            buf: h5e_buf {
                data: x.as_bytes().as_ptr(),
                len: x.len() as size_t,
            },
            marker: ContravariantLifetime,
        }
    }

    pub fn from_iobuf<'a, Buf: Iobuf>(buf: &'a Buf) -> LifetimeBuf<'a> {
        unsafe {
            let window = buf.as_window_slice().repr();
            LifetimeBuf {
                buf: h5e_buf {
                    data: window.data,
                    len:  window.len as size_t,
                },
                marker: ContravariantLifetime,
            }
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

pub struct LifetimeBufSet<'a> {
    bufset: Option<Vec<h5e_buf>>,
    marker: ContravariantLifetime<'a>,
}

impl<'a> LifetimeBufSet<'a> {
    pub fn from_span<'a>(span: &'a Span) -> LifetimeBufSet<'a> {
        LifetimeBufSet {
            bufset: Some(span.iter().map(|b| LifetimeBuf::from_iobuf(b).get()).collect()),
            marker: ContravariantLifetime,
        }
    }

    pub fn null() -> LifetimeBufSet<'static> {
        LifetimeBufSet {
            bufset: None,
            marker: ContravariantLifetime,
        }
    }

    pub fn get(&self) -> h5e_bufset {
        match self.bufset {
            None =>
                h5e_bufset::null(),
            Some(ref bufset) => {
                let as_repr = bufset.as_slice().repr();
                h5e_bufset {
                    data: as_repr.data,
                    len:  as_repr.len as size_t,
                }
            }
        }
    }
}

// Or we could just make `LifetimeBuf::from_str` generic over <T: Str>;
// see rust-lang/rust#16738.
pub trait AsLifetimeBuf {
    fn as_lifetime_buf<'a>(&'a self) -> LifetimeBuf<'a>;
}

impl AsLifetimeBuf for Atom {
    fn as_lifetime_buf<'a>(&'a self) -> LifetimeBuf<'a> {
        LifetimeBuf::from_str(self.as_slice())
    }
}

impl<'b> AsLifetimeBuf for MaybeOwned<'b> {
    fn as_lifetime_buf<'a>(&'a self) -> LifetimeBuf<'a> {
        LifetimeBuf::from_str(self.as_slice())
    }
}

pub trait AsLifetimeBufSet {
    fn as_lifetime_bufset<'a>(&'a self) -> LifetimeBufSet<'a>;
}

impl AsLifetimeBufSet for Span {
    fn as_lifetime_bufset<'a>(&'a self) -> LifetimeBufSet<'a> {
        LifetimeBufSet::from_span(self)
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
