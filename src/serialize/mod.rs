// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::io::{self, Write};
use std::default::Default;

use string_cache::{Atom, QualName};

//§ serializing-html-fragments
#[derive(Copy, Clone, PartialEq)]
pub enum TraversalScope {
    IncludeNode,
    ChildrenOnly
}

pub trait Serializable {
    fn serialize<'wr, Wr: Write>(&self, serializer: &mut Serializer<'wr, Wr>,
                                  traversal_scope: TraversalScope) -> io::Result<()>;
}

pub fn serialize<Wr: Write, T: Serializable>
    (writer: &mut Wr, node: &T, opts: SerializeOpts) -> io::Result<()> {

    let mut ser = Serializer::new(writer, opts);
    node.serialize(&mut ser, opts.traversal_scope)
}

#[derive(Copy, Clone)]
pub struct SerializeOpts {
    /// Is scripting enabled?
    pub scripting_enabled: bool,

    /// Serialize the root node? Default: ChildrenOnly
    pub traversal_scope: TraversalScope,
}

impl Default for SerializeOpts {
    fn default() -> SerializeOpts {
        SerializeOpts {
            scripting_enabled: true,
            traversal_scope: TraversalScope::ChildrenOnly,
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

fn tagname(name: &QualName) -> Atom {
    match name.ns {
        ns!(html) | ns!(mathml) | ns!(svg) => (),
        ref ns => {
            // FIXME(#122)
            warn!("node with weird namespace {:?}", &*ns.0);
        }
    }

    name.local.clone()
}

impl<'wr, Wr: Write> Serializer<'wr, Wr> {
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

    fn write_escaped(&mut self, text: &str, attr_mode: bool) -> io::Result<()> {
        for c in text.chars() {
            try!(match c {
                '&' => self.writer.write_all(b"&amp;"),
                '\u{00A0}' => self.writer.write_all(b"&nbsp;"),
                '"' if attr_mode => self.writer.write_all(b"&quot;"),
                '<' if !attr_mode => self.writer.write_all(b"&lt;"),
                '>' if !attr_mode => self.writer.write_all(b"&gt;"),
                c => self.writer.write_fmt(format_args!("{}", c)),
            });
        }
        Ok(())
    }

    pub fn start_elem<'a, AttrIter: Iterator<Item=AttrRef<'a>>>(
        &mut self,
        name: QualName,
        attrs: AttrIter) -> io::Result<()> {

        let html_name = match name.ns {
            ns!(html) => Some(name.local.clone()),
            _ => None,
        };

        if self.parent().ignore_children {
            self.stack.push(ElemInfo {
                html_name: html_name,
                ignore_children: true,
                processed_first_child: false,
            });
            return Ok(());
        }

        try!(self.writer.write_all(b"<"));
        try!(self.writer.write_all(tagname(&name).as_bytes()));
        for (name, value) in attrs {
            try!(self.writer.write_all(b" "));

            match name.ns {
                ns!() => (),
                ns!(xml) => try!(self.writer.write_all(b"xml:")),
                ns!(xmlns) => {
                    if name.local != atom!("xmlns") {
                        try!(self.writer.write_all(b"xmlns:"));
                    }
                }
                ns!(xlink) => try!(self.writer.write_all(b"xlink:")),
                ref ns => {
                    // FIXME(#122)
                    warn!("attr with weird namespace {:?}", &*ns.0);
                    try!(self.writer.write_all(b"unknown_namespace:"));
                }
            }

            try!(self.writer.write_all(name.local.as_bytes()));
            try!(self.writer.write_all(b"=\""));
            try!(self.write_escaped(value, true));
            try!(self.writer.write_all(b"\""));
        }
        try!(self.writer.write_all(b">"));

        let ignore_children = name.ns == ns!(html) && match name.local {
            atom!("area") | atom!("base") | atom!("basefont") | atom!("bgsound") | atom!("br")
            | atom!("col") | atom!("embed") | atom!("frame") | atom!("hr") | atom!("img")
            | atom!("input") | atom!("keygen") | atom!("link") | atom!("menuitem")
            | atom!("meta") | atom!("param") | atom!("source") | atom!("track") | atom!("wbr")
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

    pub fn end_elem(&mut self, name: QualName) -> io::Result<()> {
        let info = self.stack.pop().expect("no ElemInfo");
        if info.ignore_children {
            return Ok(());
        }

        try!(self.writer.write_all(b"</"));
        try!(self.writer.write_all(tagname(&name).as_bytes()));
        self.writer.write_all(b">")
    }

    pub fn write_text(&mut self, text: &str) -> io::Result<()> {
        let prepend_lf = text.starts_with("\n") && {
            let parent = self.parent();
            !parent.processed_first_child && match parent.html_name {
                Some(atom!("pre")) | Some(atom!("textarea")) | Some(atom!("listing")) => true,
                _ => false,
            }
        };

        if prepend_lf {
            try!(self.writer.write_all(b"\n"));
        }

        let escape = match self.parent().html_name {
            Some(atom!("style")) | Some(atom!("script")) | Some(atom!("xmp"))
            | Some(atom!("iframe")) | Some(atom!("noembed")) | Some(atom!("noframes"))
            | Some(atom!("plaintext")) => false,

            Some(atom!("noscript")) => !self.opts.scripting_enabled,

            _ => true,
        };

        if escape {
            self.write_escaped(text, false)
        } else {
            self.writer.write_all(text.as_bytes())
        }
    }

    pub fn write_comment(&mut self, text: &str) -> io::Result<()> {
        try!(self.writer.write_all(b"<!--"));
        try!(self.writer.write_all(text.as_bytes()));
        self.writer.write_all(b"-->")
    }

    pub fn write_doctype(&mut self, name: &str) -> io::Result<()> {
        try!(self.writer.write_all(b"<!DOCTYPE "));
        try!(self.writer.write_all(name.as_bytes()));
        self.writer.write_all(b">")
    }

    pub fn write_processing_instruction(&mut self, target: &str, data: &str) -> io::Result<()> {
        try!(self.writer.write_all(b"<?"));
        try!(self.writer.write_all(target.as_bytes()));
        try!(self.writer.write_all(b" "));
        try!(self.writer.write_all(data.as_bytes()));
        self.writer.write_all(b">")
    }
}
