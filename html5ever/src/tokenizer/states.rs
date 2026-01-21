// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Tokenizer states.
//!
//! This is public for use by the tokenizer tests.  Other library
//! users should not have to care about this.

pub use self::AttrValueKind::*;
pub use self::DoctypeIdKind::*;
pub use self::RawKind::*;
pub use self::ScriptEscapeKind::*;
pub use self::State::*;

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum ScriptEscapeKind {
    Escaped,
    DoubleEscaped,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum DoctypeIdKind {
    Public,
    System,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum RawKind {
    Rcdata,
    Rawtext,
    ScriptData,
    ScriptDataEscaped(ScriptEscapeKind),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum AttrValueKind {
    Unquoted,
    SingleQuoted,
    DoubleQuoted,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum State {
    /// <https://html.spec.whatwg.org/#data-state>
    Data,
    /// <https://html.spec.whatwg.org/#plaintext-state>
    Plaintext,
    /// <https://html.spec.whatwg.org/#tag-open-state>
    TagOpen,
    /// <https://html.spec.whatwg.org/#tag-open-state>
    EndTagOpen,
    /// <https://html.spec.whatwg.org/#tag-name-state>
    TagName,
    RawData(RawKind),
    RawLessThanSign(RawKind),
    RawEndTagOpen(RawKind),
    RawEndTagName(RawKind),
    ScriptDataEscapeStart(ScriptEscapeKind),
    /// <https://html.spec.whatwg.org/#script-data-escape-start-dash-state>
    ScriptDataEscapeStartDash,
    ScriptDataEscapedDash(ScriptEscapeKind),
    ScriptDataEscapedDashDash(ScriptEscapeKind),
    /// <https://html.spec.whatwg.org/#script-data-double-escape-end-state>
    ScriptDataDoubleEscapeEnd,
    /// <https://html.spec.whatwg.org/#before-attribute-name-state>
    BeforeAttributeName,
    /// <https://html.spec.whatwg.org/#attribute-name-state>
    AttributeName,
    /// <https://html.spec.whatwg.org/#after-attribute-name-state>
    AfterAttributeName,
    /// <https://html.spec.whatwg.org/#before-attribute-value-state>
    BeforeAttributeValue,
    AttributeValue(AttrValueKind),
    /// <https://html.spec.whatwg.org/#after-attribute-value-(quoted)-state>
    AfterAttributeValueQuoted,
    /// <https://html.spec.whatwg.org/#self-closing-start-tag-state>
    SelfClosingStartTag,
    /// <https://html.spec.whatwg.org/#bogus-comment-state>
    BogusComment,
    /// <https://html.spec.whatwg.org/#markup-declaration-open-state>
    MarkupDeclarationOpen,
    /// <https://html.spec.whatwg.org/#comment-start-state>
    CommentStart,
    /// <https://html.spec.whatwg.org/#comment-start-dash-state>
    CommentStartDash,
    /// <https://html.spec.whatwg.org/#comment-state>
    Comment,
    /// <https://html.spec.whatwg.org/#comment-less-than-sign-state>
    CommentLessThanSign,
    /// <https://html.spec.whatwg.org/#comment-less-than-sign-bang-state>
    CommentLessThanSignBang,
    /// <https://html.spec.whatwg.org/#comment-less-than-sign-bang-dash-state>
    CommentLessThanSignBangDash,
    /// <https://html.spec.whatwg.org/#comment-less-than-sign-bang-dash-dash-state>
    CommentLessThanSignBangDashDash,
    /// <https://html.spec.whatwg.org/#comment-end-dash-state>
    CommentEndDash,
    /// <https://html.spec.whatwg.org/#comment-end-state>
    CommentEnd,
    /// <https://html.spec.whatwg.org/#comment-end-bang-state>
    CommentEndBang,
    /// <https://html.spec.whatwg.org/#doctype-state>
    Doctype,
    /// <https://html.spec.whatwg.org/#before-doctype-name-state>
    BeforeDoctypeName,
    /// <https://html.spec.whatwg.org/#doctype-name-state>
    DoctypeName,
    /// <https://html.spec.whatwg.org/#after-doctype-name-state>
    AfterDoctypeName,
    AfterDoctypeKeyword(DoctypeIdKind),
    BeforeDoctypeIdentifier(DoctypeIdKind),
    DoctypeIdentifierDoubleQuoted(DoctypeIdKind),
    DoctypeIdentifierSingleQuoted(DoctypeIdKind),
    AfterDoctypeIdentifier(DoctypeIdKind),
    /// <https://html.spec.whatwg.org/#between-doctype-public-and-system-identifiers-state>
    BetweenDoctypePublicAndSystemIdentifiers,
    /// <https://html.spec.whatwg.org/#bogus-doctype-state>
    BogusDoctype,
    /// <https://html.spec.whatwg.org/#cdata-section-state>
    CdataSection,
    /// <https://html.spec.whatwg.org/#cdata-section-bracket-state>
    CdataSectionBracket,
    /// <https://html.spec.whatwg.org/#cdata-section-end-state>
    CdataSectionEnd,
}
