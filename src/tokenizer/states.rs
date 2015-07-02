pub use self::AttrValueKind::*;
pub use self::XmlState::*;

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum QuoteKind {
    SingleQuotes,
    DoubleQuotes,
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
    XDoctype,
    XTagName,
    XTagEmpty,
    TagAttrNameBefore,
    TagAttrName,
    TagAttrNameAfter,
    TagAttrValueBefore,
    TagAttrValue(AttrValueKind),
    BogusXComment,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Debug)]
pub enum AttrValueKind {
    Unquoted,
    SingleQuoted,
    DoubleQuoted,
}
