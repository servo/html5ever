pub use self::XTagKind::{StartXTag, EndXTag, EmptyXTag, ShortXTag};
pub use self::XToken::{DoctypeXToken, XTagToken, PIToken, CommentXToken};
pub use self::XToken::{CharacterXTokens, EOFXToken, XParseError, NullCharacterXToken};

use std::borrow::Cow;
use string_cache::{Atom, QualName};
use tendril::StrTendril;
use super::states;


#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub enum XTagKind {
    StartXTag,
    EndXTag,
    EmptyXTag,
    ShortXTag,
}

/// XML 5 Tag Token
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct XTag {
    pub kind: XTagKind,
    pub name: Atom,
    pub attrs: Vec<Attribute>
}

impl XTag {
    pub fn equiv_modulo_attr_order(&self, other: &XTag) -> bool {
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
    pub name: QualName,
    pub value: StrTendril,
}

/// A `DOCTYPE` token.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Doctype {
    pub name: Option<StrTendril>,
    pub public_id: Option<StrTendril>,
    pub system_id: Option<StrTendril>,
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

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct XPi {
    pub target: StrTendril,
    pub data: StrTendril,
}

#[derive(PartialEq, Eq, Debug)]
pub enum XToken {
    DoctypeXToken(Doctype),
    XTagToken(XTag),
    PIToken(XPi),
    CommentXToken(StrTendril),
    CharacterXTokens(StrTendril),
    EOFXToken,
    NullCharacterXToken,
    XParseError(Cow<'static, str>),
}

/// Types which can receive tokens from the tokenizer.
pub trait XTokenSink {
    /// Process a token.
    fn process_token(&mut self, token: XToken);

    /// The tokenizer will call this after emitting any start tag.
    /// This allows the tree builder to change the tokenizer's state.
    /// By default no state changes occur.
    fn query_state_change(&mut self) -> Option<states::XmlState> {
        None
    }
}
