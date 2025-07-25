// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Tokenizer states.

pub use AttrValueKind::*;
pub use DoctypeKind::*;
pub use XmlState::*;

/// Specifies either the public or system identifier from a [Document Type Declaration] (DTD).
///
/// [Document Type Declaration]: https://en.wikipedia.org/wiki/Document_type_declaration
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum DoctypeKind {
    /// The public identifier.
    Public,
    /// The system identifier.
    System,
}

/// Specifies the different states a XML tokenizer will assume during parsing.
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum XmlState {
    /// The initial state of the parser.
    ///
    /// It is equivalent to the [`Data`](https://html.spec.whatwg.org/#data-state) state of the html parser,
    /// except null codepoints do not cause errors.
    Data,
    /// Indicates that the parser has found a `<` character and will try to parse a tag.
    TagState,
    /// Indicates that the parser has consumed the `/` of a closing tag, like `</foo>`.
    EndTagState,
    /// Indicates that the parser is currently parsing the name of a closing tag, like the `foo` of `</foo>`.
    EndTagName,
    /// Indicates that the parser has finished parsing the name of a closing tag and expects a `>` to follow.
    EndTagNameAfter,
    /// Indicates that the parser has started parsing a [processing instruction] (PI).
    ///
    /// This state is reached after the initial `?` character has been consumed.
    ///
    /// [processing instruction]: https://en.wikipedia.org/wiki/Processing_Instruction
    Pi,
    /// Indicates that the parser is currently parsing the target of a [processing instruction].
    ///
    /// For example, the target of `<?xml-stylesheet type="text/xsl" href="style.xsl"?>` is `xml-stylesheet`.
    ///
    /// [processing instruction]: https://en.wikipedia.org/wiki/Processing_Instruction
    PiTarget,
    /// Indicates that the parser has finished parsing the target of a [processing instruction].
    ///
    /// [processing instruction]: https://en.wikipedia.org/wiki/Processing_Instruction
    PiTargetAfter,
    /// Indicates that the parser is currently parsing the data of a [processing instruction].
    ///
    /// The "data" refers to everything between the target and the closing `?` character.
    ///
    /// [processing instruction]: https://en.wikipedia.org/wiki/Processing_Instruction
    PiData,
    /// Indicates that the parser has parsed the closing `?` of a [processing instruction].
    ///
    /// [processing instruction]: https://en.wikipedia.org/wiki/Processing_Instruction
    PiAfter,
    /// Indicates that the parser has parsed the initial `!` of a markup declaration.
    ///
    /// Examples of such declarations include `<!ENTITY chap1 SYSTEM "chap1.xml">` or `<!-- Comment -->`.
    MarkupDecl,
    /// Indicates that the parser has parsed the start of a comment (`<!--`).
    CommentStart,
    /// Indicates that the parser has parsed the start of a comment and a `-` directly after it.
    CommentStartDash,
    /// Indicates that the parser is currently parsing the data within a comment.
    Comment,
    /// Indicates that the parser has parsed a `<` character within a comment.
    CommentLessThan,
    /// Indicates that the parser has parsed `<!` within a comment.
    CommentLessThanBang,
    /// Indicates that the parser has parsed `<!-` within a comment.
    CommentLessThanBangDash,
    /// Indicates that the parser has parsed `<!--` within a comment.
    CommentLessThanBangDashDash,
    /// Indicates that the parser has parsed two `-` characters within a comment which may or may not
    /// be the beginning of the comment end (`-->`).
    CommentEnd,
    /// Indicates that the parser has parsed a `-` character within a comment which may or may not
    /// be the beginning of the comment end (`-->`).
    CommentEndDash,
    /// Indicates that the parser has parsed `--!` within a comment which may or may not be part of the
    /// end of the comment. Comments in XML can be closed with `--!>`.
    CommentEndBang,
    /// Indicates that the parser has parsed the beginning of a CDATA section (`<![CDATA[`).
    Cdata,
    /// Indicates that the parser has parsed a `]` character within a CDATA section, which may be part of
    /// the end of the section (`]]>`).
    CdataBracket,
    /// Indicates that the parser has parsed two `]` characters within a CDATA section, which may be part of
    /// the end of the section (`]]>`).
    CdataEnd,
    /// Indicates that the parser is currently parsing the name of a tag, such as `foo` in `<foo>`.
    TagName,
    /// Indicates that the parser has parsed the `/` of a self-closing tag, such as `<foo/>`.
    TagEmpty,
    /// Indicates that the parser has finished parsing the name of a tag and is now expecting either attributes or
    /// a `>`.
    TagAttrNameBefore,
    /// Indicates that the parser is currently parsing the name of an attribute within a tag, such as
    /// `bar` in `<foo bar=baz>`.
    TagAttrName,
    /// Indicates that the parser has finished parsing the name of an attribute.
    TagAttrNameAfter,
    /// Indicates that the parser is about to parse the value of an attribute.
    TagAttrValueBefore,
    /// Indicates that the parser is currently parsing the value of an attribute, such as `baz` in
    /// `<foo bar=baz>`.
    ///
    /// Includes information about how the value is quoted, because the quotes before and after the attribute
    /// value need to match.
    TagAttrValue(AttrValueKind),
    /// Indicates that the parser has parsed the beginning of a document type definition (`<!DOCTYPE`).
    Doctype,
    /// Indicates that the parser expects to parse the name of the document type definition next.
    BeforeDoctypeName,
    /// Indicates that the parser is currently parsing the name of a document type definition, such as
    /// `html` in `<!DOCTYPE html>`.
    DoctypeName,
    /// Indicates that the parser has finished parsing the name of the document type definition and now optionally
    /// expects either a public or a system identifier.
    AfterDoctypeName,
    /// Indicates that the parser has parsed a keyword for either a public or system identifier (`PUBLIC` or `SYSTEM`).
    AfterDoctypeKeyword(DoctypeKind),
    /// Indicates that the parser is about to parse the value of a public or system identifier within
    /// a document type definition, such as `foo` in
    /// `<!DOCTYPE html PUBLIC "foo" "bar">`.
    BeforeDoctypeIdentifier(DoctypeKind),
    /// Indicates that the parser is currently parsing the value of a public or system identifier
    /// that is surrounded by double quotes , such as `foo` in
    /// `<!DOCTYPE html PUBLIC "foo" "bar">`.
    DoctypeIdentifierDoubleQuoted(DoctypeKind),
    /// Indicates that the parser is currently parsing the value of a public or system identifier
    /// that is surrounded by single quotes , such as `foo` in
    /// `<!DOCTYPE html PUBLIC 'foo' 'bar'>`.
    DoctypeIdentifierSingleQuoted(DoctypeKind),
    /// Indicates that the parser has finished parsing either a public or system identifier within a
    /// document type definition.
    AfterDoctypeIdentifier(DoctypeKind),
    /// Indicates that the parser has finished parsing a public identifier and now expects
    /// a system identifier.
    BetweenDoctypePublicAndSystemIdentifiers,
    /// Indicates that the parser is currently parsing an ill-formed document type defintion, such as
    /// `<!DOCTYPE html what-is-this>`.
    BogusDoctype,
    /// Indicates that the parser is currently parsing an ill-formed comment, such as
    /// `<? this is not what a comment should look like! >`.
    BogusComment,
    /// Interrupts the tokenizer for one single call to `step`.
    ///
    /// It is unclear whether this is still necessary ([#649](https://github.com/servo/html5ever/issues/649)).
    Quiescent,
}

/// Specifies how an attribute value is quoted, if at all.
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum AttrValueKind {
    /// A attribute value that is not surrounded by quotes, like `bar` in `foo=bar`.
    Unquoted,
    /// A attribute value that is not surrounded by quotes, like `bar` in `foo='bar'`.
    SingleQuoted,
    /// A attribute value that is not surrounded by quotes, like `bar` in `foo="bar"`.
    DoubleQuoted,
}
