/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[deriving(Eq, Clone, Hash)]
pub enum ScriptEscapeKind {
    Escaped,
    DoubleEscaped,
}

#[deriving(Eq, Clone, Hash)]
pub enum DoctypeIdKind {
    Public,
    System,
}

#[deriving(Eq, Clone, Hash)]
pub enum RawKind {
    Rcdata,
    Rawtext,
    ScriptData,
    ScriptDataEscaped(ScriptEscapeKind),
}

#[deriving(Eq, Clone, Hash)]
pub enum AttrValueKind {
    Unquoted,
    SingleQuoted,
    DoubleQuoted,
}

#[deriving(Eq, Clone, Hash)]
pub enum State {
    Data,
    Plaintext,
    TagOpen,
    EndTagOpen,
    TagName,
    RawData(RawKind),
    RawLessThanSign(RawKind),
    RawEndTagOpen(RawKind),
    RawEndTagName(RawKind),
    ScriptDataEscapeStart(ScriptEscapeKind),
    ScriptDataEscapeStartDash,
    ScriptDataEscapedDash(ScriptEscapeKind),
    ScriptDataEscapedDashDash(ScriptEscapeKind),
    ScriptDataDoubleEscapeEnd,
    BeforeAttributeName,
    AttributeName,
    AfterAttributeName,
    BeforeAttributeValue,
    AttributeValue(AttrValueKind),
    AfterAttributeValueQuoted,
    SelfClosingStartTag,
    BogusComment,
    MarkupDeclarationOpen,
    CommentStart,
    CommentStartDash,
    Comment,
    CommentEndDash,
    CommentEnd,
    CommentEndBang,
    Doctype,
    BeforeDoctypeName,
    DoctypeName,
    AfterDoctypeName,
    AfterDoctypeKeyword(DoctypeIdKind),
    BeforeDoctypeIdentifier(DoctypeIdKind),
    DoctypeIdentifierDoubleQuoted(DoctypeIdKind),
    DoctypeIdentifierSingleQuoted(DoctypeIdKind),
    AfterDoctypeIdentifier(DoctypeIdKind),
    BetweenDoctypePublicAndSystemIdentifiers,
    BogusDoctype,
    CdataSection,
}
