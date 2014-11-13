// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::prelude::*;
use core::slice::bytes::copy_memory;
use core::str;

use iobuf::{Iobuf, ROIobuf};
use phf::Map;

use util::single_char::SingleChar;
use util::span::Span;

/// The spec replaces most characters in the ISO-2022 C1 control code range
/// (U+0080 through U+009F) with these characters, based on Windows 8-bit
/// codepages.
static C1_REPLACEMENTS: [Option<&'static str>, ..32] = [
    Some("\u20ac"), None,           Some("\u201a"), Some("\u0192"),
    Some("\u201e"), Some("\u2026"), Some("\u2020"), Some("\u2021"),
    Some("\u02c6"), Some("\u2030"), Some("\u0160"), Some("\u2039"),
    Some("\u0152"), None,           Some("\u017d"), None,
    None,           Some("\u2018"), Some("\u2019"), Some("\u201c"),
    Some("\u201d"), Some("\u2022"), Some("\u2013"), Some("\u2014"),
    Some("\u02dc"), Some("\u2122"), Some("\u0161"), Some("\u203a"),
    Some("\u0153"), None,           Some("\u017e"), Some("\u0178"),
];

// The named_entities! macro is defined in html5/macros/named_entities.rs.
static NAMED_ENTITIES: Map<&'static str, (&'static str, u8)>
    = named_entities!("../../../data/entities.json");

pub fn lookup_c1_replacement(idx: uint) -> Option<SingleChar> {
    match C1_REPLACEMENTS.get(idx) {
        None | Some(&None) => None,
        Some(&Some(s))     => Some(SingleChar::new(ROIobuf::from_str(s))),
    }
}

/// An HTML named entity.
#[deriving(Show, Clone)]
pub struct NamedEntity {
    pub key:          &'static str,

    // The chars themselves.
    pub chars:        ROIobuf<'static>,

    // Either 0, 1, or 2 unicode codepoints.
    pub num_chars:    u8,
}

pub fn lookup_named_entity(span: &Span) -> Option<NamedEntity> {
    // We have to copy into a temporary stack buffer, since PhfHash doesn't know
    // that it must treat `Span`s and `str`s the same.
    let mut to_look_up = [0u8, ..64];
    let to_look_up     = to_look_up.as_mut_slice();

    let mut copied = 0u;

    for buf in span.iter() {
        // Safe because we're not copying both into and out of a buffer; only
        // out of one.
        unsafe {
            copy_memory(to_look_up.slice_from_mut(copied), buf.as_window_slice());
            copied += buf.len() as uint;
        }
    }

    let buf = str::from_utf8(to_look_up.slice_to(copied)).unwrap();

    NAMED_ENTITIES.get_entry(buf)
                  .map(|(k, &(s, n))|
                       NamedEntity {
                           key:       *k,
                           chars:     ROIobuf::from_str(s),
                           num_chars: n,
                       })
}
