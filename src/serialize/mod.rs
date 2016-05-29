// Copyright 2015 The xml5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::io::{self, Write};
use std::default::Default;

use string_cache::Atom;

use tokenizer::QName;

#[derive(Copy, Clone)]
/// Struct for setting serializer options.
pub struct SerializeOpts {
    /// Serialize the root node? Default: ChildrenOnly
    pub traversal_scope: TraversalScope,
}

#[derive(Copy, Clone, PartialEq)]
/// Enum describing whether or not to serialize the root node.
pub enum TraversalScope {
    /// Serialize only children.
    ChildrenOnly,
    /// Serialize current node and children.
    IncludeNode,
}

impl Default for SerializeOpts {
    fn default() -> SerializeOpts {
        SerializeOpts {
            traversal_scope: TraversalScope::ChildrenOnly
        }
    }
}

/// Trait that must be implemented by Sink in order to for
/// that TreeSink to be serializaled.
pub trait Serializable {
    /// Method for serializing node into text.
    fn serialize<'wr, Wr: Write>(&self, serializer: &mut Serializer<'wr, Wr>,
                                    traversal_scope: TraversalScope) -> io::Result<()>;
}

/// Method for serializing generic node to a given writer.
pub fn serialize<Wr, T> (writer: &mut Wr, node: &T, opts: SerializeOpts)
                        -> io::Result<()>
    where Wr: Write, T: Serializable {

    let mut ser = Serializer::new(writer, opts);
    node.serialize(&mut ser, opts.traversal_scope)
}

/// Struct used for serializing nodes into a text that other XML
/// parses can read.
///
/// Serializer contains a set of functions (start_elem, end_elem...)
/// that make parsing nodes easier.
pub struct Serializer<'wr, Wr:'wr> {
    writer: &'wr mut Wr,
    opts: SerializeOpts,
}

/// Type representing a single attribute.
/// Contains qualified name and value to attribute respectivelly.
pub type AttrRef<'a> = (&'a QName, &'a str);

fn qual_name(name: &QName) -> String {
    let mut qual_name = String::new();

    if name.prefix != Atom::from("") {
        qual_name.push_str(&*name.prefix);
        qual_name.push(':');
        qual_name.push_str(&*name.local);
    } else {
        qual_name.push_str(&*name.local);
    }

    qual_name
}


impl<'wr, Wr:Write> Serializer<'wr,Wr> {
    /// Creates a new Serializier from a writer and given serialization options.
    pub fn new(writer: &'wr mut Wr, opts: SerializeOpts) -> Serializer<'wr, Wr> {
        Serializer {
            writer: writer,
            opts: opts,
        }
    }

    /// Writes given text into the Serializer, escaping it,
    /// depending on where the text is written inside the tag or attribute value.
    ///
    /// For example
    ///```
    ///    <tag>'&-quotes'</tag>   becomes      <tag>'&amp;-quotes'</tag>
    ///    <tag = "'&-quotes'">    becomes      <tag = "&apos;&amp;-quotes&apos;"
    ///```
    fn write_escaped(&mut self, text: &str, attr_mode: bool) -> io::Result<()> {
        for c in text.chars() {
            try!(match c {
                '&' => self.writer.write_all(b"&amp;"),
                '\'' if attr_mode => self.writer.write_all(b"&apos;"),
                '"' if attr_mode => self.writer.write_all(b"&quot;"),
                '<' if !attr_mode => self.writer.write_all(b"&lt;"),
                '>' if !attr_mode => self.writer.write_all(b"&gt;"),
                c => self.writer.write_fmt(format_args!("{}", c)),
            });
        }
        Ok(())
    }

    /// Serializes given start element into text. Start element contains
    /// qualified name and an attributes iterator.
    pub fn start_elem<'a, AttrIter: Iterator<Item=AttrRef<'a>>>(
        &mut self,
        name: QName,
        attrs: AttrIter) -> io::Result<()> {

        try!(self.writer.write_all(b"<"));
        try!(self.writer.write_all(qual_name(&name).as_bytes()));
        for (name, value) in attrs {
            try!(self.writer.write_all(b" "));

            try!(self.writer.write_all(qual_name(&name).as_bytes()));
            try!(self.writer.write_all(b"=\""));
            try!(self.write_escaped(value, true));
            try!(self.writer.write_all(b"\""))

        }
        try!(self.writer.write_all(b">"));

        Ok(())
    }

    /// Serializes given end element into text.
    pub fn end_elem(&mut self, name: QName) -> io::Result<()> {
        try!(self.writer.write_all(b"</"));
        try!(self.writer.write_all(qual_name(&name).as_bytes()));
        self.writer.write_all(b">")
    }

    /// Serializes comment into text.
    pub fn write_comment(&mut self, text: &str) -> io::Result<()> {
        try!(self.writer.write_all(b"<!--"));
        try!(self.writer.write_all(text.as_bytes()));
        self.writer.write_all(b"-->")
    }

    /// Serializes given doctype
    pub fn write_doctype(&mut self, name: &str) -> io::Result<()> {
        try!(self.writer.write_all(b"<!DOCTYPE "));
        try!(self.writer.write_all(name.as_bytes()));
        self.writer.write_all(b">")
    }

    /// Serializes text for a node or an attributes.
    pub fn write_text(&mut self, text: &str) -> io::Result<()> {
        self.write_escaped(text, false)
    }

    /// Serializes given processing instruction.
    pub fn write_processing_instruction(&mut self, target: &str, data: &str) -> io::Result<()> {
        try!(self.writer.write_all(b"<?"));
        try!(self.writer.write_all(target.as_bytes()));
        try!(self.writer.write_all(b" "));
        try!(self.writer.write_all(data.as_bytes()));
        self.writer.write_all(b"?>")
    }

}
