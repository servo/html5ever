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
use {Prefix, Namespace, LocalName};
use tendril::StrTendril;
use super::{states};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
/// Fully qualified name. Used to depict names of tags and attributes.
///
/// Used to differentiate between similar XML fragments. For example
/// ```ignore
///    // HTML
///    <table>
///      <tr>
///        <td>Apples</td>
///        <td>Bananas</td>
///      </tr>
///    </table>
///
///    // Furniture XML
///    <table>
///      <name>African Coffee Table</name>
///      <width>80</width>
///      <length>120</length>
///    </table>
/// ```
/// Without XML namespaces we can't use those two fragments in occur
/// XML at same time. however if we declare a namespace we could instead say:
///
/// ```ignore
///    // Furniture XML
///    <furn:table>
///      <furn:name>African Coffee Table</furn:name>
///      <furn:width>80</furn:width>
///      <furn:length>120</furn:length>
///    </furn:table>
/// ```
/// and bind it to a different name.
///
/// For this reason we parse names that contain a colon in the following way
///
/// ```ignore
///    < furn:table>
///        |    |
///        |    +- local name
///        |
///      prefix (when resolved gives namespace_url)
/// ```
pub struct QName {
    /// Prefix of fully qualified name, used for namespace lookup.
    pub prefix: Prefix,
    /// Local name of a value.
    pub local: LocalName,
    /// Resolved namespace of `QName`.
    pub namespace_url: Namespace,
}

impl QName {
    /// Constructs a new `QName` from prefix and local part.
    /// Namespace is set to empty.
    pub fn new(prefix: Prefix, local: LocalName) -> QName {
        QName {
            prefix: prefix,
            local: local,
            namespace_url: ns!(),
        }
    }
    /// Constructs a new `QName` with only local part.
    /// Namespace is set to empty.
    pub fn new_empty(local: LocalName) -> QName {
        QName {
            prefix: namespace_prefix!(""),
            local: local,
            namespace_url: ns!(),
        }
    }
}

/// Tag kind denotes which kind of tag did we encounter.
#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub enum TagKind {
    /// Beginning of a tag (e.g. `<a>`).
    StartTag,
    /// End of a tag (e.g. `</a>`).
    EndTag,
    /// Empty tag (e.g. `<a/>`).
    EmptyTag,
    /// Short tag (e.g. `</>`).
    ShortTag,
}

/// XML 5 Tag Token
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Tag {
    /// Token kind denotes which type of token was encountered.
    /// E.g. if parser parsed `</a>` the token kind would be `EndTag`.
    pub kind: TagKind,
    /// Qualified name of the tag.
    pub name: QName,
    /// List of attributes attached to this tag.
    /// Only valid in start and empty tag.
    pub attrs: Vec<Attribute>,
}

impl Tag {
    /// Sorts attributes in a tag.
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
    /// Qualified name of attribute.
    pub name: QName,

    /// Attribute's value.
    pub value: StrTendril,
}

/// A `DOCTYPE` token.
/// Doctype token in XML5 is rather limited for reasons, such as:
/// security and simplicity. XML5 only supports declaring DTD with
/// name, public identifier and system identifier
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Doctype {
    /// Name of DOCTYPE declared
    pub name: Option<StrTendril>,
    /// Public identifier of this DOCTYPE.
    pub public_id: Option<StrTendril>,
    /// System identifier of this DOCTYPE.
    pub system_id: Option<StrTendril>,
}

impl Doctype {
    /// Constructs an empty DOCTYPE, with all fields set to None.
    pub fn new() -> Doctype {
        Doctype {
            name: None,
            public_id: None,
            system_id: None,
        }
    }
}

/// A ProcessingInstruction token.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Pi {
    /// What is the name of processing instruction.
    pub target: StrTendril,

    /// Text of processing instruction.
    pub data: StrTendril,
}

/// Describes tokens encountered during parsing of input.
#[derive(PartialEq, Eq, Debug)]
pub enum Token {
    /// Doctype token
    DoctypeToken(Doctype),
    /// Token tag founds. This token applies to all
    /// possible kinds of tags (like start, end, empty tag, etc.).
    TagToken(Tag),
    /// Processing Instruction token
    PIToken(Pi),
    /// Comment token.
    CommentToken(StrTendril),
    /// Token that represents a series of characters.
    CharacterTokens(StrTendril),
    /// End of File found.
    EOFToken,
    /// NullCharacter encountered.
    NullCharacterToken,
    /// Error happened
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
