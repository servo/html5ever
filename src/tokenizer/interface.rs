// Copyright 2014 The HTML5 for Rust Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::string::String;
use util::atom::Atom;
use tokenizer::states;

// FIXME: already exists in Servo DOM
#[deriving(Eq, TotalEq, Clone, Show)]
pub struct Doctype {
    pub name: Option<String>,
    pub public_id: Option<String>,
    pub system_id: Option<String>,
    pub force_quirks: bool,
}

impl Doctype {
    pub fn new() -> Doctype {
        Doctype {
            name: None,
            public_id: None,
            system_id: None,
            force_quirks: false,
        }
    }
}

/// Attribute name; will eventually support namespaces.
#[deriving(Eq, TotalEq, Ord, TotalOrd, Clone, Show)]
pub struct AttrName {
    name: Atom,
}

impl AttrName {
    pub fn new(name: Atom) -> AttrName {
        AttrName {
            name: name,
        }
    }
}

impl Str for AttrName {
    fn as_slice<'t>(&'t self) -> &'t str {
        self.name.as_slice()
    }
}

#[deriving(Eq, TotalEq, Clone, Show)]
pub struct Attribute {
    pub name: AttrName,
    pub value: String,
}

#[deriving(Eq, TotalEq, Clone, Show)]
pub enum TagKind {
    StartTag,
    EndTag,
}

#[deriving(Eq, TotalEq, Clone, Show)]
pub struct Tag {
    pub kind: TagKind,
    pub name: Atom,
    pub self_closing: bool,
    pub attrs: Vec<Attribute>,
}

#[deriving(Eq, TotalEq, Clone, Show)]
pub enum Token {
    DoctypeToken(Doctype),
    TagToken(Tag),
    CommentToken(String),
    CharacterTokens(String),
    NullCharacterToken,
    EOFToken,
    ParseError(String),
}

pub trait TokenSink {
    /// Process a token.
    fn process_token(&mut self, token: Token);

    /// The tokenizer will call this after emitting any start tag.
    /// This allows the tree builder to change the tokenizer's state.
    fn query_state_change(&mut self) -> Option<states::State> {
        None
    }
}
