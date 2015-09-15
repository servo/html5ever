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
    XData,
    XTagState,
    EndXTagState,
    EndXTagName,
    EndXTagNameAfter,
    Pi,
    PiTarget,
    PiTargetAfter,
    PiData,
    PiAfter,
    MarkupDecl,
    XComment,
    XCommentDash,
    XCommentEnd,
    Cdata,
    CdataBracket,
    CdataEnd,
    XTagName,
    XTagEmpty,
    TagAttrNameBefore,
    TagAttrName,
    TagAttrNameAfter,
    TagAttrValueBefore,
    TagAttrValue(AttrValueKind),
    XDoctype,
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
    BogusXComment,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum AttrValueKind {
    Unquoted,
    SingleQuoted,
    DoubleQuoted,
}
