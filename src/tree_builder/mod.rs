/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

// FIXME
#![allow(unused_imports)]

pub use self::interface::{QuirksMode, Quirks, LimitedQuirks, NoQuirks};
pub use self::interface::TreeSink;

use tokenizer::{Doctype, Attribute, AttrName, TagKind, StartTag, EndTag, Tag};
use tokenizer::{Token, DoctypeToken, TagToken, CommentToken};
use tokenizer::{CharacterTokens, EOFToken, ParseError};
use tokenizer::TokenSink;

use util::str::{strip_leading_whitespace, none_as_empty};

use std::default::Default;

mod interface;
mod states;
mod data;

/// Tree builder options, with an impl for Default.
#[deriving(Clone)]
pub struct TreeBuilderOpts {
    /// Is scripting enabled?
    pub scripting_enabled: bool,

    /// Is this an iframe srcdoc document?
    pub iframe_srcdoc: bool,
}

impl Default for TreeBuilderOpts {
    fn default() -> TreeBuilderOpts {
        TreeBuilderOpts {
            scripting_enabled: true,
            iframe_srcdoc: false,
        }
    }
}

pub struct TreeBuilder<'sink, Handle, Sink> {
    /// Options controlling the behavior of the tree builder.
    opts: TreeBuilderOpts,

    /// Consumer of tree modifications.
    sink: &'sink mut Sink,

    /// Insertion mode.
    mode: states::InsertionMode,

    /// Original insertion mode, used by Text and InTableText modes.
    orig_mode: states::InsertionMode,

    /// The document node, which is created by the sink.
    doc_handle: Handle,

    /// Stack of open elements, most recently added at end.
    open_elems: Vec<Handle>,

    /// Head element pointer.
    head_elem: Option<Handle>,
}

/// Remove leading whitespace from character tokens;
/// return None if the entire string is removed.
fn drop_whitespace(token: Token) -> Option<Token> {
    match token {
        CharacterTokens(ref x) => match strip_leading_whitespace(x.as_slice()) {
            "" => return None,
            // FIXME: We don't absolutely need to copy here, but it's hard
            // to correctly handle reconsumption without it.
            y if y.len() != x.len() => return Some(CharacterTokens(y.to_strbuf())),
            _ => (),
        },
        _ => (),
    }
    // Fall through to un-borrow `token`.
    Some(token)
}

macro_rules! drop_whitespace ( ($x:expr) => (
    unwrap_or_return!(drop_whitespace($x), ())
))

impl<'sink, Handle: Clone, Sink: TreeSink<Handle>> TreeBuilder<'sink, Handle, Sink> {
    pub fn new(sink: &'sink mut Sink, opts: TreeBuilderOpts) -> TreeBuilder<'sink, Handle, Sink> {
        let doc_handle = sink.get_document();
        TreeBuilder {
            opts: opts,
            sink: sink,
            mode: states::Initial,
            orig_mode: states::Initial,
            doc_handle: doc_handle,
            open_elems: vec!(),
            head_elem: None,
        }
    }

    fn process_in_mode(&mut self, mode: states::InsertionMode, token: Token) {
        debug!("processing {} in insertion mode {:?}", token, mode);
        match mode {
            states::Initial => match drop_whitespace!(token) {
                CommentToken(text) => self.sink.append_comment(self.doc_handle.clone(), text),
                DoctypeToken(dt) => {
                    let (err, quirk) = data::doctype_error_and_quirks(&dt, self.opts.iframe_srcdoc);
                    if err {
                        self.sink.parse_error(format!("Bad DOCTYPE: {}", dt));
                    }
                    let Doctype { name, public_id, system_id, force_quirks: _ } = dt;
                    self.sink.append_doctype_to_document(
                        none_as_empty(name),
                        none_as_empty(public_id),
                        none_as_empty(system_id)
                    );
                    self.sink.set_quirks_mode(quirk);

                    self.mode = states::BeforeHtml;
                }
                _ => fail!("not implemented"),
            },

              states::BeforeHtml
            | states::BeforeHead
            | states::InHead
            | states::InHeadNoscript
            | states::AfterHead
            | states::InBody
            | states::Text
            | states::InTable
            | states::InTableText
            | states::InCaption
            | states::InColumnGroup
            | states::InTableBody
            | states::InRow
            | states::InCell
            | states::InSelect
            | states::InSelectInTable
            | states::InTemplate
            | states::AfterBody
            | states::InFrameset
            | states::AfterFrameset
            | states::AfterAfterBody
            | states::AfterAfterFrameset
                => fail!("not implemented"),
        }
    }
}

impl<'sink, Handle: Clone, Sink: TreeSink<Handle>> TokenSink for TreeBuilder<'sink, Handle, Sink> {
    fn process_token(&mut self, token: Token) {
        self.process_in_mode(self.mode, token);
    }
}

test_eq!(drop_not_characters, drop_whitespace(EOFToken), Some(EOFToken))
test_eq!(drop_all_whitespace, drop_whitespace(CharacterTokens("    ".to_strbuf())), None)
test_eq!(drop_some_whitespace,
    drop_whitespace(CharacterTokens("   hello".to_strbuf())),
    Some(CharacterTokens("hello".to_strbuf())))
test_eq!(drop_no_whitespace,
    drop_whitespace(CharacterTokens("hello".to_strbuf())),
    Some(CharacterTokens("hello".to_strbuf())))

#[test]
fn empty_drop_doesnt_reallocate() {
    fn get_ptr(token: &Token) -> *u8 {
        match *token {
            CharacterTokens(ref x) => x.as_slice().as_ptr(),
            _ => fail!("not characters"),
        }
    }

    let x = CharacterTokens("hello".to_strbuf());
    let p = get_ptr(&x);
    assert_eq!(p, get_ptr(&drop_whitespace(x).unwrap()));
}
