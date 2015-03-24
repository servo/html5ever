// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use util::tendril::Tendril;
use tokenizer::Attribute;

use collections::vec::Vec;
use string_cache::QualName;

pub use self::NodeEnum::{Document, Doctype, Text, Comment, Element};

/// The different kinds of nodes in the DOM.
#[derive(Debug)]
pub enum NodeEnum {
    /// The `Document` itself.
    Document,

    /// A `DOCTYPE` with name, public id, and system id.
    Doctype(Tendril, Tendril, Tendril),

    /// A text node.
    Text(Tendril),

    /// A comment.
    Comment(Tendril),

    /// An element with attributes.
    Element(QualName, Vec<Attribute>),
}
