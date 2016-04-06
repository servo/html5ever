// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// This file is generated from rules.rs
// source SipHash: 18376801535389447229

# ! [
doc =
    "//! The tree builder rules, as a single, enormous nested match expression."
] use tree_builder::types::*; use tree_builder::tag_sets::*;
use tree_builder::actions::{NoPush, Push, TreeBuilderActions};
use tree_builder::interface::{TreeSink, Quirks, AppendNode, NextParserState};
use tokenizer::{Attribute, EndTag, StartTag, Tag};
use tokenizer::states::{Rcdata, Rawtext, ScriptData, Plaintext, Quiescent};
use util::str::is_ascii_whitespace; use std::ascii::AsciiExt;
use std::mem::replace; use std::borrow::Cow::Borrowed;
use std::borrow::ToOwned; use tendril::{StrTendril, SliceExt};
fn any_not_whitespace(x: &StrTendril) -> bool {
    x.chars().any(|c| !is_ascii_whitespace(c))
}
pub trait TreeBuilderStep {
    fn step(&mut self, mode: InsertionMode, token: Token)
    -> ProcessResult;
    fn step_foreign(&mut self, token: Token)
    -> ProcessResult;
}
#[doc(hidden)]
impl <Handle, Sink> TreeBuilderStep for super::TreeBuilder<Handle, Sink> where
 Handle: Clone, Sink: TreeSink<Handle = Handle> {
    fn step(&mut self, mode: InsertionMode, token: Token) -> ProcessResult {
        self.debug_step(mode, &token);
        match mode {
            Initial =>
            match token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => Done,
                CommentToken(text) => self.append_comment_to_doc(text),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => {
                            if !self.opts.iframe_srcdoc {
                                self.unexpected(&token);
                                self.set_quirks_mode(Quirks);
                            }
                            Reprocess(BeforeHtml, token)
                        }
                    }
                }
            },
            BeforeHtml =>
            match token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => Done,
                CommentToken(text) => self.append_comment_to_doc(text),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                {
                    self.create_root(tag.attrs);
                    self.mode = BeforeHead;
                    Done
                }
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token {
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("head"),
                                                            .. }) => false,
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("body"),
                                                            .. }) => false,
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("html"),
                                                            .. }) => false,
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("br"),
                                                            .. }) => false,
                            _ => true,
                        };
                    match (enable_wildcards, last_arm_token) {
                        (true,
                         ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                         kind: ::tokenizer::EndTag,
                                                         .. })) =>
                        self.unexpected(&tag),
                        (_, token) => {
                            self.create_root(vec!());
                            Reprocess(BeforeHead, token)
                        }
                    }
                }
            },
            BeforeHead =>
            match token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => Done,
                CommentToken(text) => self.append_comment(text),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                self.step(InBody, token),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("head"), .. }) =>
                {
                    self.head_elem = Some(self.insert_element_for(tag));
                    self.mode = InHead;
                    Done
                }
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token {
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("head"),
                                                            .. }) => false,
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("body"),
                                                            .. }) => false,
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("html"),
                                                            .. }) => false,
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("br"),
                                                            .. }) => false,
                            _ => true,
                        };
                    match (enable_wildcards, last_arm_token) {
                        (true,
                         ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                         kind: ::tokenizer::EndTag,
                                                         .. })) =>
                        self.unexpected(&tag),
                        (_, token) => {
                            self.head_elem =
                                Some(self.insert_phantom(atom!("head")));
                            Reprocess(InHead, token)
                        }
                    }
                }
            },
            InHead =>
            match token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => self.append_text(text),
                CommentToken(text) => self.append_comment(text),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                self.step(InBody, token),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("base"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("basefont"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("bgsound"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("link"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("meta"), .. }) =>
                {
                    self.insert_and_pop_element_for(tag);
                    DoneAckSelfClosing
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("title"), .. }) =>
                {
                    self.parse_raw_data(tag, Rcdata);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("noframes"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("style"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("noscript"), .. })
                => {
                    if (!self.opts.scripting_enabled) &&
                           (tag.name == atom!("noscript")) {
                        self.insert_element_for(tag);
                        self.mode = InHeadNoscript;
                    } else { self.parse_raw_data(tag, Rawtext); }
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("script"), .. })
                => {
                    let elem =
                        self.sink.create_element(qualname!(html , "script"),
                                                 tag.attrs);
                    if self.is_fragment() {
                        self.sink.mark_script_already_started(elem.clone());
                    }
                    self.insert_appropriately(AppendNode(elem.clone()), None);
                    self.open_elems.push(elem);
                    self.to_raw_text_mode(ScriptData);
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("head"), .. }) =>
                {
                    self.pop();
                    self.mode = AfterHead;
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("template"), .. })
                => {
                    self.insert_element_for(tag);
                    self.active_formatting.push(Marker);
                    self.frameset_ok = false;
                    self.mode = InTemplate;
                    self.template_modes.push(InTemplate);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("template"), .. })
                => {
                    if !self.in_html_elem_named(atom!("template")) {
                        self.unexpected(&tag);
                    } else {
                        self.generate_implied_end(thorough_implied_end);
                        self.expect_to_close(atom!("template"));
                        self.clear_active_formatting_to_marker();
                        self.template_modes.pop();
                        self.mode = self.reset_insertion_mode();
                    }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("head"), .. }) =>
                self.unexpected(&token),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token {
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("body"),
                                                            .. }) => false,
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("html"),
                                                            .. }) => false,
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("br"),
                                                            .. }) => false,
                            _ => true,
                        };
                    match (enable_wildcards, last_arm_token) {
                        (true,
                         ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                         kind: ::tokenizer::EndTag,
                                                         .. })) =>
                        self.unexpected(&tag),
                        (_, token) => {
                            self.pop();
                            Reprocess(AfterHead, token)
                        }
                    }
                }
            },
            InHeadNoscript =>
            match token {
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                self.step(InBody, token),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("noscript"), .. })
                => {
                    self.pop();
                    self.mode = InHead;
                    Done
                }
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InHead, token),
                CommentToken(_) => self.step(InHead, token),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("basefont"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("bgsound"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("link"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("meta"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("noframes"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("style"), .. }) =>
                self.step(InHead, token),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("head"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("noscript"), .. })
                => self.unexpected(&token),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token {
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("br"),
                                                            .. }) => false,
                            _ => true,
                        };
                    match (enable_wildcards, last_arm_token) {
                        (true,
                         ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                         kind: ::tokenizer::EndTag,
                                                         .. })) =>
                        self.unexpected(&tag),
                        (_, token) => {
                            self.unexpected(&token);
                            self.pop();
                            Reprocess(InHead, token)
                        }
                    }
                }
            },
            AfterHead =>
            match token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => self.append_text(text),
                CommentToken(text) => self.append_comment(text),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                self.step(InBody, token),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("body"), .. }) =>
                {
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    self.mode = InBody;
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("frameset"), .. })
                => {
                    self.insert_element_for(tag);
                    self.mode = InFrameset;
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("base"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("basefont"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("bgsound"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("link"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("meta"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("noframes"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("script"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("style"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("template"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("title"), .. }) =>
                {
                    self.unexpected(&token);
                    let head =
                        self.head_elem.as_ref().expect("no head element").clone();
                    self.push(&head);
                    let result = self.step(InHead, token);
                    self.remove_from_stack(&head);
                    result
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("template"), .. })
                => self.step(InHead, token),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("head"), .. }) =>
                self.unexpected(&token),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token {
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("body"),
                                                            .. }) => false,
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("html"),
                                                            .. }) => false,
                            ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                            kind: ::tokenizer::EndTag,
                                                            name: atom!("br"),
                                                            .. }) => false,
                            _ => true,
                        };
                    match (enable_wildcards, last_arm_token) {
                        (true,
                         ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                         kind: ::tokenizer::EndTag,
                                                         .. })) =>
                        self.unexpected(&tag),
                        (_, token) => {
                            self.insert_phantom(atom!("body"));
                            Reprocess(InBody, token)
                        }
                    }
                }
            },
            InBody =>
            match token {
                NullCharacterToken => self.unexpected(&token),
                CharacterTokens(_, text) => {
                    self.reconstruct_formatting();
                    if any_not_whitespace(&text) { self.frameset_ok = false; }
                    self.append_text(text)
                }
                CommentToken(text) => self.append_comment(text),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                {
                    self.unexpected(&tag);
                    if !self.in_html_elem_named(atom!("template")) {
                        let top = self.html_elem();
                        self.sink.add_attrs_if_missing(top, tag.attrs);
                    }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("base"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("basefont"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("bgsound"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("link"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("meta"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("noframes"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("script"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("style"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("template"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("title"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("template"), .. })
                => {
                    self.step(InHead, token)
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("body"), .. }) =>
                {
                    self.unexpected(&tag);
                    match self.body_elem() {
                        Some(ref node) if
                        self.open_elems.len() != 1 &&
                            !self.in_html_elem_named(atom!("template")) => {
                            self.frameset_ok = false;
                            self.sink.add_attrs_if_missing(node.clone(),
                                                           tag.attrs)
                        }
                        _ => { }
                    }
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("frameset"), .. })
                => {
                    self.unexpected(&tag);
                    if !self.frameset_ok { return Done; }
                    let body =
                        match self.body_elem() {
                            None => return Done,
                            Some(x) => x,
                        };
                    self.sink.remove_from_parent(body);
                    self.open_elems.truncate(1);
                    self.insert_element_for(tag);
                    self.mode = InFrameset;
                    Done
                }
                EOFToken => {
                    if !self.template_modes.is_empty() {
                        self.step(InTemplate, token)
                    } else { self.check_body_end(); self.stop_parsing() }
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("body"), .. }) =>
                {
                    if self.in_scope_named(default_scope, atom!("body")) {
                        self.check_body_end();
                        self.mode = AfterBody;
                    } else {
                        self.sink.parse_error(Borrowed("</body> with no <body> in scope"));
                    }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("html"), .. }) =>
                {
                    if self.in_scope_named(default_scope, atom!("body")) {
                        self.check_body_end();
                        Reprocess(AfterBody, token)
                    } else {
                        self.sink.parse_error(Borrowed("</html> with no <body> in scope"));
                        Done
                    }
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("address"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("article"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("aside"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("blockquote"), ..
                                                }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("center"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("details"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("dialog"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("dir"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("div"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("dl"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("fieldset"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("figcaption"), ..
                                                }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("figure"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("footer"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("header"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("hgroup"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("main"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("menu"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("nav"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("ol"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("p"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("section"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("summary"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("ul"), .. }) => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("h1"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("h2"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("h3"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("h4"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("h5"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("h6"), .. }) => {
                    self.close_p_element_in_button_scope();
                    if self.current_node_in(heading_tag) {
                        self.sink.parse_error(Borrowed("nested heading tags"));
                        self.pop();
                    }
                    self.insert_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("pre"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("listing"), .. })
                => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    self.ignore_lf = true;
                    self.frameset_ok = false;
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("form"), .. }) =>
                {
                    if self.form_elem.is_some() &&
                           !self.in_html_elem_named(atom!("template")) {
                        self.sink.parse_error(Borrowed("nested forms"));
                    } else {
                        self.close_p_element_in_button_scope();
                        let elem = self.insert_element_for(tag);
                        if !self.in_html_elem_named(atom!("template")) {
                            self.form_elem = Some(elem);
                        }
                    }
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("li"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("dd"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("dt"), .. }) => {
                    declare_tag_set!(close_list = "li");
                    declare_tag_set!(close_defn = "dd" "dt");
                    declare_tag_set!(extra_special = [ special_tag ] -
                                     "address" "div" "p");
                    let can_close: fn(::string_cache::QualName) -> bool =
                        match tag.name {
                            atom!("li") => close_list,
                            atom!("dd") | atom!("dt") => close_defn,
                            _ => unreachable!(),
                        };
                    self.frameset_ok = false;
                    let mut to_close = None;
                    for node in self.open_elems.iter().rev() {
                        let name = self.sink.elem_name(node.clone());
                        if can_close(name.clone()) {
                            to_close = Some(name.local);
                            break ;
                        }
                        if extra_special(name.clone()) { break ; }
                    }
                    match to_close {
                        Some(name) => {
                            self.generate_implied_end_except(name.clone());
                            self.expect_to_close(name);
                        }
                        None => (),
                    }
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("plaintext"), ..
                                                }) => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    self.next_tokenizer_state = Some(Plaintext);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("button"), .. })
                => {
                    if self.in_scope_named(default_scope, atom!("button")) {
                        self.sink.parse_error(Borrowed("nested buttons"));
                        self.generate_implied_end(cursory_implied_end);
                        self.pop_until_named(atom!("button"));
                    }
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("address"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("article"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("aside"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("blockquote"), ..
                                                }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("button"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("center"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("details"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("dialog"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("dir"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("div"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("dl"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("fieldset"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("figcaption"), ..
                                                }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("figure"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("footer"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("header"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("hgroup"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("listing"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("main"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("menu"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("nav"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("ol"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("pre"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("section"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("summary"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("ul"), .. }) => {
                    if !self.in_scope_named(default_scope, tag.name.clone()) {
                        self.unexpected(&tag);
                    } else {
                        self.generate_implied_end(cursory_implied_end);
                        self.expect_to_close(tag.name);
                    }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("form"), .. }) =>
                {
                    if !self.in_html_elem_named(atom!("template")) {
                        let node =
                            match self.form_elem.take() {
                                None => {
                                    self.sink.parse_error(Borrowed("Null form element pointer on </form>"));
                                    return Done;
                                }
                                Some(x) => x,
                            };
                        if !self.in_scope(default_scope,
                                          |n|
                                              self.sink.same_node(node.clone(),
                                                                  n)) {
                            self.sink.parse_error(Borrowed("Form element not in scope on </form>"));
                            return Done;
                        }
                        self.generate_implied_end(cursory_implied_end);
                        let current = self.current_node();
                        self.remove_from_stack(&node);
                        if !self.sink.same_node(current, node) {
                            self.sink.parse_error(Borrowed("Bad open element on </form>"));
                        }
                    } else {
                        if !self.in_scope_named(default_scope, atom!("form"))
                           {
                            self.sink.parse_error(Borrowed("Form element not in scope on </form>"));
                            return Done;
                        }
                        self.generate_implied_end(cursory_implied_end);
                        if !self.current_node_named(atom!("form")) {
                            self.sink.parse_error(Borrowed("Bad open element on </form>"));
                        }
                        self.pop_until_named(atom!("form"));
                    }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("p"), .. }) => {
                    if !self.in_scope_named(button_scope, atom!("p")) {
                        self.sink.parse_error(Borrowed("No <p> tag to close"));
                        self.insert_phantom(atom!("p"));
                    }
                    self.close_p_element();
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("li"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("dd"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("dt"), .. }) => {
                    let scope: fn(::string_cache::QualName) -> bool =
                        match tag.name {
                            atom!("li") => list_item_scope,
                            _ => default_scope,
                        };
                    if self.in_scope_named(|x| scope(x), tag.name.clone()) {
                        self.generate_implied_end_except(tag.name.clone());
                        self.expect_to_close(tag.name);
                    } else {
                        self.sink.parse_error(Borrowed("No matching tag to close"));
                    }
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("h1"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("h2"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("h3"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("h4"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("h5"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("h6"), .. }) => {
                    if self.in_scope(default_scope,
                                     |n| self.elem_in(n.clone(), heading_tag))
                       {
                        self.generate_implied_end(cursory_implied_end);
                        if !self.current_node_named(tag.name) {
                            self.sink.parse_error(Borrowed("Closing wrong heading tag"));
                        }
                        self.pop_until(heading_tag);
                    } else {
                        self.sink.parse_error(Borrowed("No heading tag to close"));
                    }
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("a"), .. }) => {
                    self.handle_misnested_a_tags(&tag);
                    self.reconstruct_formatting();
                    self.create_formatting_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("b"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("big"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("code"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("em"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("font"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("i"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("s"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("small"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("strike"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("strong"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tt"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("u"), .. }) => {
                    self.reconstruct_formatting();
                    self.create_formatting_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("nobr"), .. }) =>
                {
                    self.reconstruct_formatting();
                    if self.in_scope_named(default_scope, atom!("nobr")) {
                        self.sink.parse_error(Borrowed("Nested <nobr>"));
                        self.adoption_agency(atom!("nobr"));
                        self.reconstruct_formatting();
                    }
                    self.create_formatting_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("a"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("b"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("big"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("code"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("em"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("font"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("i"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("nobr"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("s"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("small"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("strike"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("strong"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tt"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("u"), .. }) => {
                    self.adoption_agency(tag.name);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("applet"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("marquee"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("object"), .. })
                => {
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    self.active_formatting.push(Marker);
                    self.frameset_ok = false;
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("applet"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("marquee"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("object"), .. })
                => {
                    if !self.in_scope_named(default_scope, tag.name.clone()) {
                        self.unexpected(&tag);
                    } else {
                        self.generate_implied_end(cursory_implied_end);
                        self.expect_to_close(tag.name);
                        self.clear_active_formatting_to_marker();
                    }
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("table"), .. }) =>
                {
                    if self.quirks_mode != Quirks {
                        self.close_p_element_in_button_scope();
                    }
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    self.mode = InTable;
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("br"), .. }) => {
                    self.unexpected(&tag);
                    self.step(InBody,
                              TagToken(Tag{kind: StartTag,
                                           attrs: vec!(), ..tag}))
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("area"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("br"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("embed"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("img"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("keygen"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("wbr"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("input"), .. }) =>
                {
                    let keep_frameset_ok =
                        match tag.name {
                            atom!("input") => self.is_type_hidden(&tag),
                            _ => false,
                        };
                    self.reconstruct_formatting();
                    self.insert_and_pop_element_for(tag);
                    if !keep_frameset_ok { self.frameset_ok = false; }
                    DoneAckSelfClosing
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("menuitem"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("param"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("source"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("track"), .. }) =>
                {
                    self.insert_and_pop_element_for(tag);
                    DoneAckSelfClosing
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("hr"), .. }) => {
                    self.close_p_element_in_button_scope();
                    self.insert_and_pop_element_for(tag);
                    self.frameset_ok = false;
                    DoneAckSelfClosing
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("image"), .. }) =>
                {
                    self.unexpected(&tag);
                    self.step(InBody,
                              TagToken(Tag{name: atom!("img"), ..tag}))
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("isindex"), .. })
                => {
                    self.unexpected(&tag);
                    let in_template =
                        self.in_html_elem_named(atom!("template"));
                    if !in_template && self.form_elem.is_some() {
                        return Done;
                    }
                    self.frameset_ok = false;
                    self.close_p_element_in_button_scope();
                    let mut form_attrs = vec!();
                    let mut prompt = None;
                    let mut input_attrs = vec!();
                    for attr in tag.attrs.into_iter() {
                        match attr.name {
                            qualname!("" , "action") => form_attrs.push(attr),
                            qualname!("" , "prompt") =>
                            prompt = Some(attr.value),
                            qualname!("" , "name") => { }
                            _ => input_attrs.push(attr),
                        }
                    }
                    input_attrs.push(Attribute{name: qualname!("" , "name"),
                                               value:
                                                   "isindex".to_tendril(),});
                    let form =
                        self.insert_element(Push, ns!(html), atom!("form"),
                                            form_attrs);
                    if !in_template { self.form_elem = Some(form.clone()); }
                    self.insert_element(NoPush, ns!(html), atom!("hr"),
                                        vec!());
                    self.reconstruct_formatting();
                    self.insert_element(Push, ns!(html), atom!("label"),
                                        vec!());
                    self.append_text(prompt.unwrap_or_else(|| {
                                                           "This is a searchable index. Enter search keywords: ".to_tendril()
                                                       }));
                    self.insert_element(NoPush, ns!(html), atom!("input"),
                                        input_attrs);
                    self.pop();
                    self.insert_element(NoPush, ns!(html), atom!("hr"),
                                        vec!());
                    self.pop();
                    if !in_template { self.form_elem = None; }
                    DoneAckSelfClosing
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("textarea"), .. })
                => {
                    self.ignore_lf = true;
                    self.frameset_ok = false;
                    self.parse_raw_data(tag, Rcdata);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("xmp"), .. }) => {
                    self.close_p_element_in_button_scope();
                    self.reconstruct_formatting();
                    self.frameset_ok = false;
                    self.parse_raw_data(tag, Rawtext);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("iframe"), .. })
                => {
                    self.frameset_ok = false;
                    self.parse_raw_data(tag, Rawtext);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("noembed"), .. })
                => {
                    self.parse_raw_data(tag, Rawtext);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("select"), .. })
                => {
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    self.mode =
                        match self.mode {
                            InTable | InCaption | InTableBody | InRow | InCell
                            => InSelectInTable,
                            _ => InSelect,
                        };
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("optgroup"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("option"), .. })
                => {
                    if self.current_node_named(atom!("option")) {
                        self.pop();
                    }
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("rb"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("rtc"), .. }) => {
                    if self.in_scope_named(default_scope, atom!("ruby")) {
                        self.generate_implied_end(cursory_implied_end);
                    }
                    if !self.current_node_named(atom!("ruby")) {
                        self.unexpected(&tag);
                    }
                    self.insert_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("rp"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("rt"), .. }) => {
                    if self.in_scope_named(default_scope, atom!("ruby")) {
                        self.generate_implied_end_except(atom!("rtc"));
                    }
                    if !self.current_node_named(atom!("rtc")) &&
                           !self.current_node_named(atom!("ruby")) {
                        self.unexpected(&tag);
                    }
                    self.insert_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("math"), .. }) =>
                self.enter_foreign(tag, ns!(mathml)),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("svg"), .. }) =>
                self.enter_foreign(tag, ns!(svg)),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("col"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("colgroup"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("frame"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("head"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("th"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("thead"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tr"), .. }) => {
                    self.unexpected(&token);
                    Done
                }
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (true,
                         ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                         kind: ::tokenizer::StartTag,
                                                         .. })) => {
                            if self.opts.scripting_enabled &&
                                   tag.name == atom!("noscript") {
                                self.parse_raw_data(tag, Rawtext);
                            } else {
                                self.reconstruct_formatting();
                                self.insert_element_for(tag);
                            }
                            Done
                        }
                        (true,
                         ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                         kind: ::tokenizer::EndTag,
                                                         .. })) => {
                            self.process_end_tag_in_body(tag);
                            Done
                        }
                        (_, _) => panic!("impossible case in InBody mode"),
                    }
                }
            },
            Text =>
            match token {
                CharacterTokens(_, text) => self.append_text(text),
                EOFToken => {
                    self.unexpected(&token);
                    if self.current_node_named(atom!("script")) {
                        let current = self.current_node();
                        self.sink.mark_script_already_started(current);
                    }
                    self.pop();
                    Reprocess(self.orig_mode.take().unwrap(), token)
                }
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (true,
                         ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                         kind: ::tokenizer::EndTag,
                                                         .. })) => {
                            let node = self.pop();
                            if tag.name == atom!("script") {
                                warn!("FIXME: </script> not fully implemented");
                                if self.sink.complete_script(node) ==
                                       NextParserState::Suspend {
                                    self.next_tokenizer_state =
                                        Some(Quiescent);
                                }
                            }
                            self.mode = self.orig_mode.take().unwrap();
                            Done
                        }
                        (_, _) => panic!("impossible case in Text mode"),
                    }
                }
            },
            InTable =>
            match token {
                NullCharacterToken => self.process_chars_in_table(token),
                CharacterTokens(..) => self.process_chars_in_table(token),
                CommentToken(text) => self.append_comment(text),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("caption"), .. })
                => {
                    self.pop_until_current(table_scope);
                    self.active_formatting.push(Marker);
                    self.insert_element_for(tag);
                    self.mode = InCaption;
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("colgroup"), .. })
                => {
                    self.pop_until_current(table_scope);
                    self.insert_element_for(tag);
                    self.mode = InColumnGroup;
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("col"), .. }) => {
                    self.pop_until_current(table_scope);
                    self.insert_phantom(atom!("colgroup"));
                    Reprocess(InColumnGroup, token)
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("thead"), .. }) =>
                {
                    self.pop_until_current(table_scope);
                    self.insert_element_for(tag);
                    self.mode = InTableBody;
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("th"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tr"), .. }) => {
                    self.pop_until_current(table_scope);
                    self.insert_phantom(atom!("tbody"));
                    Reprocess(InTableBody, token)
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("table"), .. }) =>
                {
                    self.unexpected(&token);
                    if self.in_scope_named(table_scope, atom!("table")) {
                        self.pop_until_named(atom!("table"));
                        Reprocess(self.reset_insertion_mode(), token)
                    } else { Done }
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("table"), .. }) =>
                {
                    if self.in_scope_named(table_scope, atom!("table")) {
                        self.pop_until_named(atom!("table"));
                        self.mode = self.reset_insertion_mode();
                    } else { self.unexpected(&token); }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("body"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("col"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("colgroup"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("html"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("th"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("thead"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tr"), .. }) =>
                self.unexpected(&token),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("style"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("script"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("template"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("template"), .. })
                => self.step(InHead, token),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("input"), .. }) =>
                {
                    self.unexpected(&tag);
                    if self.is_type_hidden(&tag) {
                        self.insert_and_pop_element_for(tag);
                        DoneAckSelfClosing
                    } else { self.foster_parent_in_body(TagToken(tag)) }
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("form"), .. }) =>
                {
                    self.unexpected(&tag);
                    if !self.in_html_elem_named(atom!("template")) &&
                           self.form_elem.is_none() {
                        self.form_elem =
                            Some(self.insert_and_pop_element_for(tag));
                    }
                    Done
                }
                EOFToken => self.step(InBody, token),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => {
                            self.unexpected(&token);
                            self.foster_parent_in_body(token)
                        }
                    }
                }
            },
            InTableText =>
            match token {
                NullCharacterToken => self.unexpected(&token),
                CharacterTokens(split, text) => {
                    self.pending_table_text.push((split, text));
                    Done
                }
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => {
                            let pending =
                                replace(&mut self.pending_table_text, vec!());
                            let contains_nonspace =
                                pending.iter().any(|&(split, ref text)| {
                                                   match split {
                                                       Whitespace => false,
                                                       NotWhitespace => true,
                                                       NotSplit =>
                                                       any_not_whitespace(text),
                                                   } });
                            if contains_nonspace {
                                self.sink.parse_error(Borrowed("Non-space table text"));
                                for (split, text) in pending.into_iter() {
                                    match self.foster_parent_in_body(CharacterTokens(split,
                                                                                     text))
                                        {
                                        Done => (),
                                        _ =>
                                        panic!("not prepared to handle this!"),
                                    }
                                }
                            } else {
                                for (_, text) in pending.into_iter() {
                                    self.append_text(text);
                                }
                            }
                            Reprocess(self.orig_mode.take().unwrap(), token)
                        }
                    }
                }
            },
            InCaption =>
            match token {
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("col"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("colgroup"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("th"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("thead"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tr"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("table"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("caption"), .. })
                => {
                    if self.in_scope_named(table_scope, atom!("caption")) {
                        self.generate_implied_end(cursory_implied_end);
                        self.expect_to_close(atom!("caption"));
                        self.clear_active_formatting_to_marker();
                        match tag {
                            Tag { kind: EndTag, name: atom!("caption"), .. }
                            => {
                                self.mode = InTable;
                                Done
                            }
                            _ => Reprocess(InTable, TagToken(tag)),
                        }
                    } else { self.unexpected(&tag); Done }
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("body"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("col"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("colgroup"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("html"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("th"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("thead"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tr"), .. }) =>
                self.unexpected(&token),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => self.step(InBody, token),
                    }
                }
            },
            InColumnGroup =>
            match token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => self.append_text(text),
                CommentToken(text) => self.append_comment(text),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                self.step(InBody, token),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("col"), .. }) => {
                    self.insert_and_pop_element_for(tag);
                    DoneAckSelfClosing
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("colgroup"), .. })
                => {
                    if self.current_node_named(atom!("colgroup")) {
                        self.pop();
                        self.mode = InTable;
                    } else { self.unexpected(&token); }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("col"), .. }) =>
                self.unexpected(&token),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("template"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("template"), .. })
                => self.step(InHead, token),
                EOFToken => self.step(InBody, token),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => {
                            if self.current_node_named(atom!("colgroup")) {
                                self.pop();
                                Reprocess(InTable, token)
                            } else { self.unexpected(&token) }
                        }
                    }
                }
            },
            InTableBody =>
            match token {
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tr"), .. }) => {
                    self.pop_until_current(table_body_context);
                    self.insert_element_for(tag);
                    self.mode = InRow;
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("th"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("td"), .. }) => {
                    self.unexpected(&token);
                    self.pop_until_current(table_body_context);
                    self.insert_phantom(atom!("tr"));
                    Reprocess(InRow, token)
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("thead"), .. }) =>
                {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.pop_until_current(table_body_context);
                        self.pop();
                        self.mode = InTable;
                    } else { self.unexpected(&tag); }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("col"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("colgroup"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("thead"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("table"), .. }) =>
                {
                    declare_tag_set!(table_outer = "table" "tbody" "tfoot");
                    if self.in_scope(table_scope,
                                     |e| self.elem_in(e, table_outer)) {
                        self.pop_until_current(table_body_context);
                        self.pop();
                        Reprocess(InTable, token)
                    } else { self.unexpected(&token) }
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("body"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("col"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("colgroup"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("html"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("th"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tr"), .. }) =>
                self.unexpected(&token),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => self.step(InTable, token),
                    }
                }
            },
            InRow =>
            match token {
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("th"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("td"), .. }) => {
                    self.pop_until_current(table_row_context);
                    self.insert_element_for(tag);
                    self.mode = InCell;
                    self.active_formatting.push(Marker);
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tr"), .. }) => {
                    if self.in_scope_named(table_scope, atom!("tr")) {
                        self.pop_until_current(table_row_context);
                        let node = self.pop();
                        self.assert_named(node, atom!("tr"));
                        self.mode = InTableBody;
                    } else { self.unexpected(&token); }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("col"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("colgroup"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("thead"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tr"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("table"), .. }) =>
                {
                    if self.in_scope_named(table_scope, atom!("tr")) {
                        self.pop_until_current(table_row_context);
                        let node = self.pop();
                        self.assert_named(node, atom!("tr"));
                        Reprocess(InTableBody, token)
                    } else { self.unexpected(&token) }
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("thead"), .. }) =>
                {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        if self.in_scope_named(table_scope, atom!("tr")) {
                            self.pop_until_current(table_row_context);
                            let node = self.pop();
                            self.assert_named(node, atom!("tr"));
                            Reprocess(InTableBody, TagToken(tag))
                        } else { Done }
                    } else { self.unexpected(&tag) }
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("body"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("col"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("colgroup"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("html"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("th"), .. }) =>
                self.unexpected(&token),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => self.step(InTable, token),
                    }
                }
            },
            InCell =>
            match token {
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("th"), .. }) => {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.generate_implied_end(cursory_implied_end);
                        self.expect_to_close(tag.name);
                        self.clear_active_formatting_to_marker();
                        self.mode = InRow;
                    } else { self.unexpected(&tag); }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("col"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("colgroup"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("th"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("thead"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tr"), .. }) => {
                    if self.in_scope(table_scope,
                                     |n| self.elem_in(n.clone(), td_th)) {
                        self.close_the_cell();
                        Reprocess(InRow, token)
                    } else { self.unexpected(&token) }
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("body"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("col"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("colgroup"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("html"), .. }) =>
                self.unexpected(&token),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("table"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("thead"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tr"), .. }) => {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.close_the_cell();
                        Reprocess(InRow, TagToken(tag))
                    } else { self.unexpected(&tag) }
                }
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => self.step(InBody, token),
                    }
                }
            },
            InSelect =>
            match token {
                NullCharacterToken => self.unexpected(&token),
                CharacterTokens(_, text) => self.append_text(text),
                CommentToken(text) => self.append_comment(text),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                self.step(InBody, token),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("option"), .. })
                => {
                    if self.current_node_named(atom!("option")) {
                        self.pop();
                    }
                    self.insert_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("optgroup"), .. })
                => {
                    if self.current_node_named(atom!("option")) {
                        self.pop();
                    }
                    if self.current_node_named(atom!("optgroup")) {
                        self.pop();
                    }
                    self.insert_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("optgroup"), .. })
                => {
                    if self.open_elems.len() >= 2 &&
                           self.current_node_named(atom!("option")) &&
                           self.html_elem_named(self.open_elems[self.open_elems.len()
                                                                    -
                                                                    2].clone(),
                                                atom!("optgroup")) {
                        self.pop();
                    }
                    if self.current_node_named(atom!("optgroup")) {
                        self.pop();
                    } else { self.unexpected(&token); }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("option"), .. })
                => {
                    if self.current_node_named(atom!("option")) {
                        self.pop();
                    } else { self.unexpected(&token); }
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("select"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("select"), .. })
                => {
                    let in_scope =
                        self.in_scope_named(select_scope, atom!("select"));
                    if !in_scope || tag.kind == StartTag {
                        self.unexpected(&tag);
                    }
                    if in_scope {
                        self.pop_until_named(atom!("select"));
                        self.mode = self.reset_insertion_mode();
                    }
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("input"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("keygen"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("textarea"), .. })
                => {
                    self.unexpected(&token);
                    if self.in_scope_named(select_scope, atom!("select")) {
                        self.pop_until_named(atom!("select"));
                        Reprocess(self.reset_insertion_mode(), token)
                    } else { Done }
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("script"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("template"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("template"), .. })
                => self.step(InHead, token),
                EOFToken => self.step(InBody, token),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => self.unexpected(&token),
                    }
                }
            },
            InSelectInTable =>
            match token {
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("table"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("thead"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tr"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("th"), .. }) => {
                    self.unexpected(&token);
                    self.pop_until_named(atom!("select"));
                    Reprocess(self.reset_insertion_mode(), token)
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("table"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("thead"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("tr"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("th"), .. }) => {
                    self.unexpected(&tag);
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.pop_until_named(atom!("select"));
                        Reprocess(self.reset_insertion_mode(), TagToken(tag))
                    } else { Done }
                }
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => self.step(InSelect, token),
                    }
                }
            },
            InTemplate =>
            match token {
                CharacterTokens(_, _) => self.step(InBody, token),
                CommentToken(_) => self.step(InBody, token),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("base"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("basefont"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("bgsound"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("link"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("meta"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("noframes"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("script"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("style"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("template"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("title"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("template"), .. })
                => {
                    self.step(InHead, token)
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("caption"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("colgroup"), .. })
                |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tbody"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tfoot"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("thead"), .. }) =>
                {
                    self.template_modes.pop();
                    self.template_modes.push(InTable);
                    Reprocess(InTable, token)
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("col"), .. }) => {
                    self.template_modes.pop();
                    self.template_modes.push(InColumnGroup);
                    Reprocess(InColumnGroup, token)
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("tr"), .. }) => {
                    self.template_modes.pop();
                    self.template_modes.push(InTableBody);
                    Reprocess(InTableBody, token)
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("td"), .. }) |
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("th"), .. }) => {
                    self.template_modes.pop();
                    self.template_modes.push(InRow);
                    Reprocess(InRow, token)
                }
                EOFToken => {
                    if !self.in_html_elem_named(atom!("template")) {
                        self.stop_parsing()
                    } else {
                        self.unexpected(&token);
                        self.pop_until_named(atom!("template"));
                        self.clear_active_formatting_to_marker();
                        self.template_modes.pop();
                        self.mode = self.reset_insertion_mode();
                        Reprocess(self.reset_insertion_mode(), token)
                    }
                }
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (true,
                         ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                         kind: ::tokenizer::StartTag,
                                                         .. })) => {
                            self.template_modes.pop();
                            self.template_modes.push(InBody);
                            Reprocess(InBody, TagToken(tag))
                        }
                        (_, token) => self.unexpected(&token),
                    }
                }
            },
            AfterBody =>
            match token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InBody, token),
                CommentToken(text) => self.append_comment_to_html(text),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                self.step(InBody, token),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("html"), .. }) =>
                {
                    if self.is_fragment() {
                        self.unexpected(&token);
                    } else { self.mode = AfterAfterBody; }
                    Done
                }
                EOFToken => self.stop_parsing(),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => {
                            self.unexpected(&token);
                            Reprocess(InBody, token)
                        }
                    }
                }
            },
            InFrameset =>
            match token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => self.append_text(text),
                CommentToken(text) => self.append_comment(text),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                self.step(InBody, token),
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("frameset"), .. })
                => {
                    self.insert_element_for(tag);
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("frameset"), .. })
                => {
                    if self.open_elems.len() == 1 {
                        self.unexpected(&token);
                    } else {
                        self.pop();
                        if !self.is_fragment() &&
                               !self.current_node_named(atom!("frameset")) {
                            self.mode = AfterFrameset;
                        }
                    }
                    Done
                }
                ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("frame"), .. }) =>
                {
                    self.insert_and_pop_element_for(tag);
                    DoneAckSelfClosing
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("noframes"), .. })
                => self.step(InHead, token),
                EOFToken => {
                    if self.open_elems.len() != 1 { self.unexpected(&token); }
                    self.stop_parsing()
                }
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => self.unexpected(&token),
                    }
                }
            },
            AfterFrameset =>
            match token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => self.append_text(text),
                CommentToken(text) => self.append_comment(text),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                self.step(InBody, token),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::EndTag,
                                                name: atom!("html"), .. }) =>
                {
                    self.mode = AfterAfterFrameset;
                    Done
                }
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("noframes"), .. })
                => self.step(InHead, token),
                EOFToken => self.stop_parsing(),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => self.unexpected(&token),
                    }
                }
            },
            AfterAfterBody =>
            match token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InBody, token),
                CommentToken(text) => self.append_comment_to_doc(text),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                self.step(InBody, token),
                EOFToken => self.stop_parsing(),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => {
                            self.unexpected(&token);
                            Reprocess(InBody, token)
                        }
                    }
                }
            },
            AfterAfterFrameset =>
            match token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InBody, token),
                CommentToken(text) => self.append_comment_to_doc(text),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("html"), .. }) =>
                self.step(InBody, token),
                EOFToken => self.stop_parsing(),
                ::tree_builder::types::TagToken(::tokenizer::Tag {
                                                kind: ::tokenizer::StartTag,
                                                name: atom!("noframes"), .. })
                => self.step(InHead, token),
                last_arm_token => {
                    let enable_wildcards =
                        match last_arm_token { _ => true, };
                    match (enable_wildcards, last_arm_token) {
                        (_, token) => self.unexpected(&token),
                    }
                }
            },
        }
    }
    fn step_foreign(&mut self, token: Token) -> ProcessResult {
        match token {
            NullCharacterToken => {
                self.unexpected(&token);
                self.append_text("\u{fffd}".to_tendril())
            }
            CharacterTokens(_, text) => {
                if any_not_whitespace(&text) { self.frameset_ok = false; }
                self.append_text(text)
            }
            CommentToken(text) => self.append_comment(text),
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("b"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("big"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("blockquote"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("body"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("br"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("center"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("code"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("dd"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("div"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("dl"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("dt"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("em"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("embed"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("h1"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("h2"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("h3"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("h4"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("h5"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("h6"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("head"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("hr"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("i"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("img"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("li"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("listing"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("menu"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("meta"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("nobr"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("ol"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("p"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("pre"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("ruby"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("s"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("small"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("span"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("strong"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("strike"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("sub"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("sup"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("table"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("tt"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("u"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("ul"), .. }) |
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("var"), .. }) =>
            self.unexpected_start_tag_in_foreign_content(tag),
            ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                            kind: ::tokenizer::StartTag,
                                            name: atom!("font"), .. }) => {
                let unexpected =
                    tag.attrs.iter().any(|attr| {
                                         matches!(attr . name , qualname ! (
                                                  "" , "color" ) | qualname !
                                                  ( "" , "face" ) | qualname !
                                                  ( "" , "size" )) });
                if unexpected {
                    self.unexpected_start_tag_in_foreign_content(tag)
                } else { self.foreign_start_tag(tag) }
            }
            last_arm_token => {
                let enable_wildcards = match last_arm_token { _ => true, };
                match (enable_wildcards, last_arm_token) {
                    (true,
                     ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                     kind: ::tokenizer::StartTag,
                                                     .. })) =>
                    self.foreign_start_tag(tag),
                    (true,
                     ::tree_builder::types::TagToken(tag@::tokenizer::Tag {
                                                     kind: ::tokenizer::EndTag,
                                                     .. })) => {
                        let mut first = true;
                        let mut stack_idx = self.open_elems.len() - 1;
                        loop  {
                            if stack_idx == 0 { return Done; }
                            let node = self.open_elems[stack_idx].clone();
                            let node_name = self.sink.elem_name(node);
                            if !first && node_name.ns == ns!(html) {
                                let mode = self.mode;
                                return self.step(mode, TagToken(tag));
                            }
                            if (&*node_name.local).eq_ignore_ascii_case(&*tag.name)
                               {
                                self.open_elems.truncate(stack_idx);
                                return Done;
                            }
                            if first { self.unexpected(&tag); first = false; }
                            stack_idx -= 1;
                        }
                    }
                    (_, _) => panic!("impossible case in foreign content"),
                }
            }
        }
    }
}