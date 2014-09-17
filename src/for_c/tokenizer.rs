// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_camel_case_types)]

use core::prelude::*;

use for_c::common::{LifetimeBuf, AsLifetimeBuf, h5e_buf, c_bool};

use tokenizer::{TokenSink, Token, Doctype, Tag, ParseError, DoctypeToken};
use tokenizer::{CommentToken, CharacterTokens, NullCharacterToken};
use tokenizer::{TagToken, StartTag, EndTag, EOFToken, Tokenizer};

use core::mem;
use core::default::Default;
use alloc::boxed::Box;
use collections::String;
use libc::{c_void, c_int, size_t};

#[repr(C)]
pub struct h5e_token_ops {
    do_doctype: Option<extern "C" fn(user: *mut c_void, name: h5e_buf,
        public: h5e_buf, system: h5e_buf, force_quirks: c_int)>,

    do_start_tag: Option<extern "C" fn(user: *mut c_void, name: h5e_buf,
        self_closing: c_int, num_attrs: size_t)>,

    do_tag_attr:      Option<extern "C" fn(user: *mut c_void, name: h5e_buf, value: h5e_buf)>,
    do_end_tag:       Option<extern "C" fn(user: *mut c_void, name: h5e_buf)>,
    do_comment:       Option<extern "C" fn(user: *mut c_void, text: h5e_buf)>,
    do_chars:         Option<extern "C" fn(user: *mut c_void, text: h5e_buf)>,
    do_null_char:     Option<extern "C" fn(user: *mut c_void)>,
    do_eof:           Option<extern "C" fn(user: *mut c_void)>,
    do_error:         Option<extern "C" fn(user: *mut c_void, message: h5e_buf)>,
}

#[repr(C)]
pub struct h5e_token_sink {
    ops: *const h5e_token_ops,
    user: *mut c_void,
}

impl TokenSink for h5e_token_sink {
    fn process_token(&mut self, token: Token) {
        macro_rules! call ( ($name:ident $(, $arg:expr)*) => (
            unsafe {
                match (*self.ops).$name {
                    None => (),
                    Some(f) => f(self.user $(, $arg)*),
                }
            }
        ))

        fn opt_str_to_buf<'a>(s: &'a Option<String>) -> LifetimeBuf<'a> {
            match *s {
                None => LifetimeBuf::null(),
                Some(ref s) => s.as_lifetime_buf(),
            }
        }

        match token {
            DoctypeToken(Doctype { name, public_id, system_id, force_quirks }) => {
                let name = opt_str_to_buf(&name);
                let public_id = opt_str_to_buf(&public_id);
                let system_id = opt_str_to_buf(&system_id);
                call!(do_doctype, name.get(), public_id.get(), system_id.get(),
                    c_bool(force_quirks));
            }

            TagToken(Tag { kind, name, self_closing, attrs }) => {
                let name = name.as_lifetime_buf();
                match kind {
                    StartTag => {
                        call!(do_start_tag, name.get(), c_bool(self_closing),
                            attrs.len() as size_t);
                        for attr in attrs.into_iter() {
                            let name = attr.name.name.as_lifetime_buf();
                            let value = attr.value.as_lifetime_buf();
                            call!(do_tag_attr, name.get(), value.get());
                        }
                    }
                    EndTag => call!(do_end_tag, name.get()),
                }
            }

            CommentToken(text) => {
                let text = text.as_lifetime_buf();
                call!(do_comment, text.get());
            }

            CharacterTokens(text) => {
                let text = text.as_lifetime_buf();
                call!(do_chars, text.get());
            }

            NullCharacterToken => call!(do_null_char),

            EOFToken => call!(do_eof),

            ParseError(msg) => {
                let msg = msg.as_lifetime_buf();
                call!(do_error, msg.get());
            }
        }
    }
}

pub type h5e_tokenizer_ptr = *const ();

#[no_mangle]
pub unsafe extern "C" fn h5e_tokenizer_new(sink: *mut h5e_token_sink) -> h5e_tokenizer_ptr {
    let tok: Box<Tokenizer<h5e_token_sink>>
        = box Tokenizer::new(mem::transmute::<_, &mut h5e_token_sink>(sink),
            Default::default());

    mem::transmute(tok)
}

#[no_mangle]
pub unsafe extern "C" fn h5e_tokenizer_free(tok: h5e_tokenizer_ptr) {
    let _: Box<Tokenizer<h5e_token_sink>> = mem::transmute(tok);
}

#[no_mangle]
pub unsafe extern "C" fn h5e_tokenizer_feed(tok: h5e_tokenizer_ptr, buf: h5e_buf) {
    let tok: &mut Tokenizer<h5e_token_sink> = mem::transmute(tok);
    tok.feed(buf.with_slice(|s| String::from_str(s)));
}

#[no_mangle]
pub unsafe extern "C" fn h5e_tokenizer_end(tok: h5e_tokenizer_ptr) {
    let tok: &mut Tokenizer<h5e_token_sink> = mem::transmute(tok);
    tok.end();
}
