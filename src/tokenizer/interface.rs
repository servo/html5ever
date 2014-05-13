/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::strbuf::StrBuf;
use util::atom::Atom;

// FIXME: already exists in Servo DOM
#[deriving(Eq, TotalEq, Clone, Show)]
pub struct Doctype {
    pub name: Option<StrBuf>,
    pub public_id: Option<StrBuf>,
    pub system_id: Option<StrBuf>,
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

#[deriving(Eq, TotalEq, Clone, Show)]
pub struct Attribute {
    pub name: Atom,
    pub value: StrBuf,
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
    CommentToken(StrBuf),
    CharacterToken(char),
    MultiCharacterToken(StrBuf),
    EOFToken,
    ParseError(~str),
}

pub trait TokenSink {
    fn process_token(&mut self, token: Token);
}
