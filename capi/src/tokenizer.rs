// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_camel_case_types, raw_pointer_derive)]

use c_bool;

use html5ever::tokenizer::{TokenSink, Token, Doctype, Tag, ParseError, DoctypeToken};
use html5ever::tokenizer::{CommentToken, CharacterTokens, NullCharacterToken};
use html5ever::tokenizer::{TagToken, StartTag, EndTag, EOFToken, Tokenizer};

use std::mem;
use std::default::Default;

use libc::{c_void, c_int, size_t};
use string_cache::Atom;
use tendril::{StrTendril, SliceExt};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct h5e_token_ops {
    do_doctype: Option<extern "C" fn(user: *mut c_void, name: StrTendril,
        public: StrTendril, system: StrTendril, force_quirks: c_int)>,

    do_start_tag: Option<extern "C" fn(user: *mut c_void, name: Atom,
        self_closing: c_int, num_attrs: size_t)>,

    do_tag_attr: Option<extern "C" fn(user: *mut c_void, name: Atom, value: StrTendril)>,

    do_end_tag:       Option<extern "C" fn(user: *mut c_void, name: Atom)>,
    do_comment:       Option<extern "C" fn(user: *mut c_void, text: StrTendril)>,
    do_chars:         Option<extern "C" fn(user: *mut c_void, text: StrTendril)>,
    do_null_char:     Option<extern "C" fn(user: *mut c_void)>,
    do_eof:           Option<extern "C" fn(user: *mut c_void)>,
    do_error:         Option<extern "C" fn(user: *mut c_void, message: StrTendril)>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct h5e_token_sink {
    ops: *const h5e_token_ops,
    user: *mut c_void,
}

impl TokenSink for h5e_token_sink {
    fn process_token(&mut self, token: Token) {
        macro_rules! call {
            ($name:ident, $($arg:expr),*) => (
                unsafe {
                    match (*self.ops).$name {
                        None => (),
                        Some(f) => f((*self).user $(, $arg)*),
                    }
                }
            );
            ($name:ident) => (call!($name,)); // bleh
        }

        match token {
            DoctypeToken(Doctype { name, public_id, system_id, force_quirks }) => {
                // Empty tendril doesn't allocate.
                call!(do_doctype, name.unwrap_or(StrTendril::new()),
                    public_id.unwrap_or(StrTendril::new()),
                    system_id.unwrap_or(StrTendril::new()),
                    c_bool(force_quirks));
            }

            TagToken(Tag { kind, name, self_closing, attrs }) => {
                match kind {
                    StartTag => {
                        call!(do_start_tag, name, c_bool(self_closing),
                            attrs.len() as size_t);
                        for attr in attrs.into_iter() {
                            // All attribute names from the tokenizer are local.
                            assert!(attr.name.ns == ns!(""));
                            call!(do_tag_attr, attr.name.local, attr.value);
                        }
                    }
                    EndTag => call!(do_end_tag, name),
                }
            }

            CommentToken(text) => call!(do_comment, text),

            CharacterTokens(text) => call!(do_chars, text),

            NullCharacterToken => call!(do_null_char),

            EOFToken => call!(do_eof),

            ParseError(msg) => {
                let msg = msg.to_tendril();
                call!(do_error, msg);
            }
        }
    }
}

pub type h5e_tokenizer_ptr = *const ();

#[no_mangle]
pub unsafe extern "C" fn h5e_tokenizer_new(sink: *const h5e_token_sink) -> h5e_tokenizer_ptr {
    let tok: Box<Tokenizer<h5e_token_sink>>
        = box Tokenizer::new(*sink, Default::default());

    mem::transmute(tok)
}

#[no_mangle]
pub unsafe extern "C" fn h5e_tokenizer_free(tok: h5e_tokenizer_ptr) {
    let _: Box<Tokenizer<h5e_token_sink>> = mem::transmute(tok);
}

#[no_mangle]
pub unsafe extern "C" fn h5e_tokenizer_feed(tok: h5e_tokenizer_ptr, buf: StrTendril) {
    let tok: &mut Tokenizer<h5e_token_sink> = mem::transmute(tok);
    tok.feed(buf);
}

#[no_mangle]
pub unsafe extern "C" fn h5e_tokenizer_end(tok: h5e_tokenizer_ptr) {
    let tok: &mut Tokenizer<h5e_token_sink> = mem::transmute(tok);
    tok.end();
}
