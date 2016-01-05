// Copyright 2015 The xml5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub use self::AttrValueKind::*;
pub use self::XmlState::*;
pub use self::DoctypeKind::*;

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum DoctypeKind {
    Public,
    System,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum XmlState {
    Data,
    TagState,
    EndTagState,
    EndTagName,
    EndTagNameAfter,
    Pi,
    PiTarget,
    PiTargetAfter,
    PiData,
    PiAfter,
    MarkupDecl,
    Comment,
    CommentDash,
    CommentEnd,
    Cdata,
    CdataBracket,
    CdataEnd,
    TagName,
    TagEmpty,
    TagAttrNameBefore,
    TagAttrName,
    TagAttrNameAfter,
    TagAttrValueBefore,
    TagAttrValue(AttrValueKind),
    Doctype,
    BeforeDoctypeName,
    DoctypeName,
    AfterDoctypeName,
    AfterDoctypeKeyword(DoctypeKind),
    BeforeDoctypeIdentifier(DoctypeKind),
    DoctypeIdentifierDoubleQuoted(DoctypeKind),
    DoctypeIdentifierSingleQuoted(DoctypeKind),
    AfterDoctypeIdentifier(DoctypeKind),
    BetweenDoctypePublicAndSystemIdentifiers,
    BogusDoctype,
    BogusComment,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum AttrValueKind {
    Unquoted,
    SingleQuoted,
    DoubleQuoted,
}
