// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use tokenizer::Attribute;

use collections::vec::Vec;
use collections::string::String;
use string_cache::QualName;

pub use self::NodeEnum::{Document, Doctype, Text, Comment, Element};

/// The different kinds of nodes in the DOM.
#[derive(Show)]
pub enum NodeEnum {
    /// The `Document` itself.
    Document,

    /// A `DOCTYPE` with name, public id, and system id.
    Doctype(String, String, String),

    /// A text node.
    Text(String),

    /// A comment.
    Comment(String),

    /// An element with attributes.
    Element(QualName, Vec<Attribute>),
}
