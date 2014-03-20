/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[deriving(Eq)]
pub enum ScriptEscapeKind {
    Escaped,
    DoubleEscaped,
}

#[deriving(Eq)]
pub enum DoctypeIdKind {
    Public,
    System,
}

#[deriving(Eq)]
pub enum RawKind {
    Rcdata,
    Rawtext,
    ScriptData,
    ScriptDataEscaped(ScriptEscapeKind),
}

#[deriving(Eq)]
pub enum State {
    Data,
    CharacterReferenceInData,
    CharacterReferenceInRcdata,
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
    AttributeValueDoubleQuoted,
    AttributeValueSingleQuoted,
    AttributeValueUnquoted,
    CharacterReferenceInAttributeValue,
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
