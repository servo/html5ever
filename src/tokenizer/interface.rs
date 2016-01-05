// Copyright 2015 The xml5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub use self::TagKind::{StartTag, EndTag, EmptyTag, ShortTag};
pub use self::Token::{DoctypeToken, TagToken, PIToken, CommentToken};
pub use self::Token::{CharacterTokens, EOFToken, ParseError, NullCharacterToken};

use std::borrow::Cow;
use string_cache::{Atom};
use tendril::StrTendril;
use super::{states};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct QName {
    pub prefix: Atom,
    pub local: Atom,
    pub namespace_url: Atom,
}

impl QName {
    pub fn new(prefix: Atom, local: Atom) -> QName {
        QName {
            prefix: prefix,
            local: local,
            namespace_url: Atom::from(""),
        }
    }
    /// Constructs a new `QName` with only local part.
    /// Namespace is set to empty atom.
    pub fn new_empty(local: Atom) -> QName {
        QName {
            prefix: Atom::from(""),
            local: local,
            namespace_url: Atom::from(""),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub enum TagKind {
    StartTag,
    EndTag,
    EmptyTag,
    ShortTag,
}

/// XML 5 Tag Token
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Tag {
    pub kind: TagKind,
    pub name: QName,
    pub attrs: Vec<Attribute>,
}

impl Tag {
    pub fn equiv_modulo_attr_order(&self, other: &Tag) -> bool {
        if (self.kind != other.kind) || (self.name != other.name) {
            return false;
        }

        let mut self_attrs = self.attrs.clone();
        let mut other_attrs = other.attrs.clone();
        self_attrs.sort();
        other_attrs.sort();

        self_attrs == other_attrs
    }
}

/// A tag attribute.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct Attribute {
    pub name: QName,
    pub value: StrTendril,
}

/// A `DOCTYPE` token.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Doctype {
    pub name: Option<StrTendril>,
    pub public_id: Option<StrTendril>,
    pub system_id: Option<StrTendril>,
}

impl Doctype {
    pub fn new() -> Doctype {
        Doctype {
            name: None,
            public_id: None,
            system_id: None,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Pi {
    pub target: StrTendril,
    pub data: StrTendril,
}

#[derive(PartialEq, Eq, Debug)]
pub enum Token {
    DoctypeToken(Doctype),
    TagToken(Tag),
    PIToken(Pi),
    CommentToken(StrTendril),
    CharacterTokens(StrTendril),
    EOFToken,
    NullCharacterToken,
    ParseError(Cow<'static, str>),
}

/// Types which can receive tokens from the tokenizer.
pub trait TokenSink {
    /// Process a token.
    fn process_token(&mut self, token: Token);

    /// The tokenizer will call this after emitting any start tag.
    /// This allows the tree builder to change the tokenizer's state.
    /// By default no state changes occur.
    fn query_state_change(&mut self) -> Option<states::XmlState> {
        None
    }
}
