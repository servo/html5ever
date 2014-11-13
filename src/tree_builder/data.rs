// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::prelude::*;

use tokenizer::Doctype;
use tree_builder::interface::{QuirksMode, Quirks, LimitedQuirks, NoQuirks};
use util::span::Span;
use util::str::AsciiCast;

use iobuf::Iobuf;
use string_cache::atom::Atom;

// These should all be lowercase, for ASCII-case-insensitive matching.
static QUIRKY_PUBLIC_PREFIXES: &'static [&'static [u8]] = &[
    b"-//advasoft ltd//dtd html 3.0 aswedit + extensions//",
    b"-//as//dtd html 3.0 aswedit + extensions//",
    b"-//ietf//dtd html 2.0 level 1//",
    b"-//ietf//dtd html 2.0 level 2//",
    b"-//ietf//dtd html 2.0 strict level 1//",
    b"-//ietf//dtd html 2.0 strict level 2//",
    b"-//ietf//dtd html 2.0 strict//",
    b"-//ietf//dtd html 2.0//",
    b"-//ietf//dtd html 2.1e//",
    b"-//ietf//dtd html 3.0//",
    b"-//ietf//dtd html 3.2 final//",
    b"-//ietf//dtd html 3.2//",
    b"-//ietf//dtd html 3//",
    b"-//ietf//dtd html level 0//",
    b"-//ietf//dtd html level 1//",
    b"-//ietf//dtd html level 2//",
    b"-//ietf//dtd html level 3//",
    b"-//ietf//dtd html strict level 0//",
    b"-//ietf//dtd html strict level 1//",
    b"-//ietf//dtd html strict level 2//",
    b"-//ietf//dtd html strict level 3//",
    b"-//ietf//dtd html strict//",
    b"-//ietf//dtd html//",
    b"-//metrius//dtd metrius presentational//",
    b"-//microsoft//dtd internet explorer 2.0 html strict//",
    b"-//microsoft//dtd internet explorer 2.0 html//",
    b"-//microsoft//dtd internet explorer 2.0 tables//",
    b"-//microsoft//dtd internet explorer 3.0 html strict//",
    b"-//microsoft//dtd internet explorer 3.0 html//",
    b"-//microsoft//dtd internet explorer 3.0 tables//",
    b"-//netscape comm. corp.//dtd html//",
    b"-//netscape comm. corp.//dtd strict html//",
    b"-//o'reilly and associates//dtd html 2.0//",
    b"-//o'reilly and associates//dtd html extended 1.0//",
    b"-//o'reilly and associates//dtd html extended relaxed 1.0//",
    b"-//softquad software//dtd hotmetal pro 6.0::19990601::extensions to html 4.0//",
    b"-//softquad//dtd hotmetal pro 4.0::19971010::extensions to html 4.0//",
    b"-//spyglass//dtd html 2.0 extended//",
    b"-//sq//dtd html 2.0 hotmetal + extensions//",
    b"-//sun microsystems corp.//dtd hotjava html//",
    b"-//sun microsystems corp.//dtd hotjava strict html//",
    b"-//w3c//dtd html 3 1995-03-24//",
    b"-//w3c//dtd html 3.2 draft//",
    b"-//w3c//dtd html 3.2 final//",
    b"-//w3c//dtd html 3.2//",
    b"-//w3c//dtd html 3.2s draft//",
    b"-//w3c//dtd html 4.0 frameset//",
    b"-//w3c//dtd html 4.0 transitional//",
    b"-//w3c//dtd html experimental 19960712//",
    b"-//w3c//dtd html experimental 970421//",
    b"-//w3c//dtd w3 html//",
    b"-//w3o//dtd w3 html 3.0//",
    b"-//webtechs//dtd mozilla html 2.0//",
    b"-//webtechs//dtd mozilla html//",
];

static QUIRKY_PUBLIC_MATCHES: &'static [&'static [u8]] = &[
    b"-//w3o//dtd w3 html strict 3.0//en//",
    b"-/w3c/dtd html 4.0 transitional/en",
    b"html",
];

static QUIRKY_SYSTEM_MATCHES: &'static [&'static [u8]] = &[
    b"http://www.ibm.com/data/dtd/v11/ibmxhtml1-transitional.dtd",
];

static LIMITED_QUIRKY_PUBLIC_PREFIXES: &'static [&'static [u8]] = &[
    b"-//w3c//dtd xhtml 1.0 frameset//",
    b"-//w3c//dtd xhtml 1.0 transitional//",
];

static HTML4_PUBLIC_PREFIXES: &'static [&'static [u8]] = &[
    b"-//w3c//dtd html 4.01 frameset//",
    b"-//w3c//dtd html 4.01 transitional//",
];

pub fn doctype_error_and_quirks(doctype: &Doctype, iframe_srcdoc: bool) -> (bool, QuirksMode) {
    fn byte_equal(s: &Option<Span>, b: &[u8]) -> bool {
        s.as_ref().map(|s| s.byte_equal_slice(b)).unwrap_or(b.is_empty())
    }

    fn atom_byte_equal(a: &Option<Atom>, b: &[u8]) -> bool {
        a.as_ref().map(|a| a.as_slice().as_bytes() == b.as_slice()).unwrap_or(b.is_empty())
    }

    fn is_doctype_ok(doctype: &Doctype) -> bool {
        let name   = &doctype.name;
        let public = &doctype.public_id;
        let system = &doctype.system_id;

        let has_system_id = system.is_some();

        if !atom_byte_equal(name, b"html") {
            false
        } else if !public.is_some() {
            !has_system_id || byte_equal(system, b"about:legacy-compat")
        } else if byte_equal(public, b"-//W3C//DTD HTML 4.0//EN") {
            !has_system_id || byte_equal(system, b"http://www.w3.org/TR/REC-html40/strict.dtd")
        } else if byte_equal(public, b"-//W3C//DTD HTML 4.01//EN") {
            !has_system_id || byte_equal(system, b"http://www.w3.org/TR/html4/strict.dtd")
        } else if byte_equal(public, b"-//W3C//DTD XHTML 1.0 Strict//EN") {
            byte_equal(system, b"http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd")
        } else if byte_equal(public, b"-//W3C//DTD XHTML 1.1//EN") {
            byte_equal(system, b"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd")
        } else {
            false
        }
    }

    let err = !is_doctype_ok(doctype);

    fn contains_span(haystack: &[&[u8]], needle: &Span) -> bool {
        let needle_len = needle.count_bytes() as uint;
        haystack.iter().any(|&x|
            needle_len == x.len()
            && needle.iter_bytes().zip(x.iter()).all(
                // Quirks-mode matches are case-insensitive.
                |(a, b)| match (a.to_ascii_opt(), b.to_ascii_opt()) {
                    (Some(a), Some(b)) => a.eq_ignore_case(b),
                    (None, None) => true,
                    _ => false,
                    }))
    }

    // FIXME: We could do something asymptotically faster here.
    // But there aren't many strings, and this happens at most once per parse.
    fn contains_span_prefix(haystack: &[&[u8]], needle: &Span) -> bool {
        let needle_len = needle.count_bytes() as uint;
        haystack.iter().any(|&x|
            needle_len >= x.len()
            && needle.iter_bytes().zip(x.iter()).all(
                // Quirks-mode matches are case-insensitive.
                |(a, b)| match (a.to_ascii_opt(), b.to_ascii_opt()) {
                    (Some(a), Some(b)) => a.eq_ignore_case(b),
                    (None, None) => true,
                    _ => false,
                    }))
    }

    let quirk = match (doctype.public_id.as_ref(), doctype.system_id.as_ref()) {
        _ if doctype.force_quirks => Quirks,
        _ if !atom_byte_equal(&doctype.name, b"html") => Quirks,

        _ if iframe_srcdoc => NoQuirks,

        (Some(p), _) if contains_span(QUIRKY_PUBLIC_MATCHES, p) => Quirks,
        (_, Some(s)) if contains_span(QUIRKY_SYSTEM_MATCHES, s) => Quirks,

        (Some(p), _) if contains_span_prefix(QUIRKY_PUBLIC_PREFIXES, p) => Quirks,
        (Some(p), _) if contains_span_prefix(LIMITED_QUIRKY_PUBLIC_PREFIXES, p) => LimitedQuirks,

        (Some(p), s) if contains_span_prefix(HTML4_PUBLIC_PREFIXES, p) => match s {
            None => Quirks,
            Some(_) => LimitedQuirks,
        },

        _ => NoQuirks,
    };

    (err, quirk)
}
