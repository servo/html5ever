// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use QualName;
use std::io;

//§ serializing-html-fragments
#[derive(Copy, Clone, PartialEq)]
pub enum TraversalScope {
    IncludeNode,
    ChildrenOnly
}

pub trait Serialize {
    fn serialize<S>(&self, serializer: &mut S, traversal_scope: TraversalScope) -> io::Result<()>
    where S: Serializer;
}

pub trait Serializer {
    fn start_elem<'a, AttrIter>(&mut self, name: QualName, attrs: AttrIter) -> io::Result<()>
    where AttrIter: Iterator<Item=AttrRef<'a>>;

    fn end_elem(&mut self, name: QualName) -> io::Result<()>;

    fn write_text(&mut self, text: &str) -> io::Result<()>;

    fn write_comment(&mut self, text: &str) -> io::Result<()>;

    fn write_doctype(&mut self, name: &str) -> io::Result<()>;

    fn write_processing_instruction(&mut self, target: &str, data: &str) -> io::Result<()>;
}

pub type AttrRef<'a> = (&'a QualName, &'a str);
