// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use util::atom::Atom;
use tokenizer::Attribute;

use collections::vec::Vec;
use collections::string::String;

/// The different kinds of nodes in the DOM.
#[deriving(Show)]
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
    ///
    /// FIXME: HTML namespace only for now.
    Element(Atom, Vec<Attribute>),
}

