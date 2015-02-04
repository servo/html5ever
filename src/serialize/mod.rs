// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::prelude::*;

use std::old_io::{Writer, IoResult};
use core::default::Default;
use collections::vec::Vec;

use string_cache::{Atom, QualName};

//§ serializing-html-fragments
pub trait Serializable {
    fn serialize<'wr, Wr: Writer>(&self, serializer: &mut Serializer<'wr, Wr>, incl_self: bool) -> IoResult<()>;
}

pub fn serialize<Wr: Writer, T: Serializable>
    (writer: &mut Wr, node: &T, opts: SerializeOpts) -> IoResult<()> {

    let mut ser = Serializer::new(writer, opts);
    node.serialize(&mut ser, false)
}

#[derive(Copy)]
pub struct SerializeOpts {
    /// Is scripting enabled?
    pub scripting_enabled: bool,
}

impl Default for SerializeOpts {
    fn default() -> SerializeOpts {
        SerializeOpts {
            scripting_enabled: true,
        }
    }
}

struct ElemInfo {
    html_name: Option<Atom>,
    ignore_children: bool,
    processed_first_child: bool,
}

pub type AttrRef<'a> = (&'a QualName, &'a str);

pub struct Serializer<'wr, Wr:'wr> {
    writer: &'wr mut Wr,
    opts: SerializeOpts,
    stack: Vec<ElemInfo>,
}

impl<'wr, Wr: Writer> Serializer<'wr, Wr> {
    fn new(writer: &'wr mut Wr, opts: SerializeOpts) -> Serializer<'wr, Wr> {
        Serializer {
            writer: writer,
            opts: opts,
            stack: vec!(ElemInfo {
                html_name: None,
                ignore_children: false,
                processed_first_child: false,
            }),
        }
    }

    fn parent<'a>(&'a mut self) -> &'a mut ElemInfo {
        self.stack.last_mut().expect("no parent ElemInfo")
    }

    fn write_escaped(&mut self, text: &str, attr_mode: bool) -> IoResult<()> {
        for c in text.chars() {
            try!(match c {
                '&' => self.writer.write_str("&amp;"),
                '\u{00A0}' => self.writer.write_str("&nbsp;"),
                '"' if attr_mode => self.writer.write_str("&quot;"),
                '<' if !attr_mode => self.writer.write_str("&lt;"),
                '>' if !attr_mode => self.writer.write_str("&gt;"),
                c => self.writer.write_char(c),
            });
        }
        Ok(())
    }

    pub fn start_elem<'a, AttrIter: Iterator<Item=AttrRef<'a>>>(
        &mut self,
        name: QualName,
        attrs: AttrIter) -> IoResult<()> {

        let html_name = match name.ns {
            ns!(HTML) => Some(name.local.clone()),
            _ => panic!("FIXME: Handle qualified tag names"),
        };

        if self.parent().ignore_children {
            self.stack.push(ElemInfo {
                html_name: html_name,
                ignore_children: true,
                processed_first_child: false,
            });
            return Ok(());
        }

        try!(self.writer.write_char('<'));
        try!(self.writer.write_str(name.local.as_slice()));
        for (name, value) in attrs {
            try!(self.writer.write_char(' '));
            // FIXME: qualified names
            assert!(name.ns == ns!(""));
            try!(self.writer.write_str(name.local.as_slice()));
            try!(self.writer.write_str("=\""));
            try!(self.write_escaped(value, true));
            try!(self.writer.write_char('"'));
        }
        try!(self.writer.write_char('>'));

        let ignore_children = name.ns == ns!(HTML) && match name.local {
            atom!(area) | atom!(base) | atom!(basefont) | atom!(bgsound) | atom!(br)
            | atom!(col) | atom!(embed) | atom!(frame) | atom!(hr) | atom!(img)
            | atom!(input) | atom!(keygen) | atom!(link) | atom!(menuitem)
            | atom!(meta) | atom!(param) | atom!(source) | atom!(track) | atom!(wbr)
                => true,
            _ => false,
        };

        self.parent().processed_first_child = true;

        self.stack.push(ElemInfo {
            html_name: html_name,
            ignore_children: ignore_children,
            processed_first_child: false,
        });

        Ok(())
    }

    pub fn end_elem(&mut self, name: QualName) -> IoResult<()> {
        let info = self.stack.pop().expect("no ElemInfo");
        if info.ignore_children {
            return Ok(());
        }

        // FIXME: Handle qualified tag names
        try!(self.writer.write_str("</"));
        try!(self.writer.write_str(name.local.as_slice()));
        self.writer.write_char('>')
    }

    pub fn write_text(&mut self, text: &str) -> IoResult<()> {
        let prepend_lf = text.starts_with("\n") && {
            let parent = self.parent();
            !parent.processed_first_child && match parent.html_name {
                Some(atom!(pre)) | Some(atom!(textarea)) | Some(atom!(listing)) => true,
                _ => false,
            }
        };

        if prepend_lf {
            try!(self.writer.write_char('\n'));
        }

        let escape = match self.parent().html_name {
            Some(atom!(style)) | Some(atom!(script)) | Some(atom!(xmp))
            | Some(atom!(iframe)) | Some(atom!(noembed)) | Some(atom!(noframes))
            | Some(atom!(plaintext)) => false,

            Some(atom!(noscript)) => !self.opts.scripting_enabled,

            _ => true,
        };

        if escape {
            self.write_escaped(text, false)
        } else {
            self.writer.write_str(text)
        }
    }

    pub fn write_comment(&mut self, text: &str) -> IoResult<()> {
        try!(self.writer.write_str("<!--"));
        try!(self.writer.write_str(text));
        self.writer.write_str("-->")
    }

    pub fn write_doctype(&mut self, name: &str) -> IoResult<()> {
        try!(self.writer.write_str("<!DOCTYPE "));
        try!(self.writer.write_str(name));
        self.writer.write_char('\n')
    }
}
