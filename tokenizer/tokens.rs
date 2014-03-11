use std::hashmap::HashMap;
use std::str;

// FIXME: already exists in Servo DOM
pub struct Doctype {
    name: Option<~str>,
    public_id: Option<~str>,
    system_id: Option<~str>,
    force_quirks: bool,
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


pub struct Attributes {
    data: HashMap<~str, ~str>,
}

impl Attributes {
    pub fn new() -> Attributes {
        Attributes {
            data: HashMap::new(),
        }
    }
}

pub enum TagKind {
    StartTag,
    EndTag,
}

pub struct Tag {
    kind: TagKind,
    name: ~str,
    self_closing: bool,
    attrs: Attributes,
}

impl Tag {
    pub fn new(kind: TagKind) -> Tag {
        Tag {
            kind: kind,
            name: str::with_capacity(8), // FIXME: justify this
            self_closing: false,
            attrs: Attributes::new(),
        }
    }
}


pub enum Token {
    DoctypeToken(Doctype),
    TagToken(Tag),
    CommentToken(~str),
    CharacterToken(char),
    EOFToken,
}
