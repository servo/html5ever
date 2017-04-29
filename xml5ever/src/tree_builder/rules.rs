// Copyright 2015 The xml5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::borrow::Cow::Borrowed;
use tendril::StrTendril;
use tokenizer::{Tag, StartTag, EndTag, ShortTag, EmptyTag};
use tree_builder::types::*;
use markup5ever::interface::TreeSink;
use tree_builder::actions::XmlTreeBuilderActions;

fn any_not_whitespace(x: &StrTendril) -> bool {
    !x.bytes().all(|b| matches!(b, b'\t' | b'\r' | b'\n' | b'\x0C' | b' '))
}

/// Encapsulates rules needed to build a tree representation.
pub trait XmlTreeBuilderStep {

    /// Each step presents resolving received Token, in a
    /// given XmlPhase.
    fn step(&mut self, mode: XmlPhase, token: Token) -> XmlProcessResult;
}

#[doc(hidden)]
impl<Handle, Sink> XmlTreeBuilderStep
    for super::XmlTreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{

    fn step(&mut self, mode: XmlPhase, token: Token) -> XmlProcessResult {
        self.debug_step(mode, &token);

        match mode {
            StartPhase => match token {
                TagToken(Tag{kind: StartTag, name, attrs}) => {
                    let tag = {
                        let mut tag = Tag {
                            kind: StartTag,
                            name: name,
                            attrs: attrs,
                        };
                        self.process_namespaces(&mut tag);
                        tag
                    };
                    self.phase = MainPhase;
                    let handle = self.append_tag_to_doc(tag);
                    self.add_to_open_elems(handle)
                },
                TagToken(Tag{kind: EmptyTag, name, attrs}) => {
                    let tag = {
                        let mut tag = Tag {
                            kind: EmptyTag,
                            name: name,
                            attrs: attrs,
                        };
                        self.process_namespaces(&mut tag);
                        tag
                    };
                    self.phase = EndPhase;
                    let handle = self.append_tag_to_doc(tag);
                    self.sink.pop(handle);
                    Done
                },
                CommentToken(comment) => {
                    self.append_comment_to_doc(comment)
                },
                PIToken(pi) => {
                    self.append_pi_to_doc(pi)
                },
                CharacterTokens(ref chars)
                    if !any_not_whitespace(chars) => {
                        Done
                },
                EOFToken => {
                    self.sink.parse_error(Borrowed("Unexpected EOF in start phase"));
                    Reprocess(EndPhase, EOFToken)
                },
                DoctypeToken(d) => {
                    self.append_doctype_to_doc(d);
                    Done
                },
                _ => {
                    self.sink.parse_error(Borrowed("Unexpected element in start phase"));
                    Done
                },
            },
            MainPhase => match token {
                CharacterTokens(chs) => {
                    self.append_text(chs)
                },
                TagToken(Tag{kind: StartTag, name, attrs}) => {
                    let tag = {
                        let mut tag = Tag {
                            kind: StartTag,
                            name: name,
                            attrs: attrs,
                        };
                        self.process_namespaces(&mut tag);
                        tag
                    };
                    self.insert_tag(tag)
                },
                TagToken(Tag{kind: EmptyTag, name, attrs}) => {
                    let tag = {
                        let mut tag = Tag {
                            kind: EmptyTag,
                            name: name,
                            attrs: attrs,
                        };
                        self.process_namespaces(&mut tag);
                        tag
                    };
                    if tag.name.local == local_name!("script") {
                        self.insert_tag(tag.clone());
                        self.complete_script();
                        self.close_tag(tag)
                    } else {
                        self.append_tag(tag)
                    }
                },
                TagToken(Tag{kind: EndTag, name, attrs}) => {
                    let tag = {
                        let mut tag = Tag {
                            kind: EndTag,
                            name: name,
                            attrs: attrs,
                        };
                        self.process_namespaces(&mut tag);
                        tag
                    };
                    if tag.name.local == local_name!("script") {
                        self.complete_script();
                    }
                    let retval = self.close_tag(tag);
                    if self.no_open_elems() {
                        self.phase = EndPhase;
                    }
                    retval
                },
                TagToken(Tag{kind: ShortTag, ..}) => {
                    self.pop();
                    if self.no_open_elems() {
                        self.phase = EndPhase;
                    }
                    Done
                },
                CommentToken(comment) => {
                    self.append_comment_to_tag(comment)
                },
                PIToken(pi) => {
                    self.append_pi_to_tag(pi)
                },
                EOFToken | NullCharacterToken=> {
                    Reprocess(EndPhase, EOFToken)
                }
                DoctypeToken(_) => {
                    self.sink.parse_error(Borrowed("Unexpected element in main phase"));
                    Done
                }
            },
            EndPhase => match token {
                CommentToken(comment) => {
                    self.append_comment_to_doc(comment)
                },
                PIToken(pi) => {
                    self.append_pi_to_doc(pi)
                },
                CharacterTokens(ref chars)
                    if !any_not_whitespace(chars) => {
                        Done
                },
                EOFToken => {
                    self.stop_parsing()
                }
                _ => {
                    self.sink.parse_error(Borrowed("Unexpected element in end phase"));
                    Done
                }
            },

        }
    }
}
