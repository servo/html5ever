// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Types used within the tree builder code.  Not exported to users.

use core::prelude::*;

use tokenizer::Tag;

use collections::string::String;

pub use self::InsertionMode::*;
pub use self::SplitStatus::*;
pub use self::Token::*;
pub use self::ProcessResult::*;
pub use self::FormatEntry::*;

#[derive(PartialEq, Eq, Copy, Clone, Show)]
pub enum InsertionMode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    InHeadNoscript,
    AfterHead,
    InBody,
    Text,
    InTable,
    InTableText,
    InCaption,
    InColumnGroup,
    InTableBody,
    InRow,
    InCell,
    InSelect,
    InSelectInTable,
    InTemplate,
    AfterBody,
    InFrameset,
    AfterFrameset,
    AfterAfterBody,
    AfterAfterFrameset,
}

#[derive(PartialEq, Eq, Copy, Clone, Show)]
pub enum SplitStatus {
    NotSplit,
    Whitespace,
    NotWhitespace,
}

/// A subset/refinement of `tokenizer::Token`.  Everything else is handled
/// specially at the beginning of `process_token`.
#[derive(PartialEq, Eq, Clone, Show)]
pub enum Token {
    TagToken(Tag),
    CommentToken(String),
    CharacterTokens(SplitStatus, String),
    NullCharacterToken,
    EOFToken,
}

pub enum ProcessResult {
    Done,
    DoneAckSelfClosing,
    SplitWhitespace(String),
    Reprocess(InsertionMode, Token),
}

pub enum FormatEntry<Handle> {
    Element(Handle, Tag),
    Marker,
}
