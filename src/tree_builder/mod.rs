// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The HTML5 tree builder.

pub use self::interface::{QuirksMode, Quirks, LimitedQuirks, NoQuirks};
pub use self::interface::{NodeOrText, AppendNode, AppendText};
pub use self::interface::TreeSink;

use self::types::*;
use self::actions::TreeBuilderActions;
use self::rules::TreeBuilderStep;

use tokenizer;
use tokenizer::{Doctype, Tag};
use tokenizer::TokenSink;

use util::namespace::HTML;
use util::str::{is_ascii_whitespace, Runs};

use std::default::Default;
use std::mem::replace;
use std::str::Slice;

mod interface;
mod tag_sets;
mod data;
mod types;
mod actions;
mod rules;

/// Tree builder options, with an impl for Default.
#[deriving(Clone)]
pub struct TreeBuilderOpts {
    /// Report all parse errors described in the spec, at some
    /// performance penalty?  Default: false
    pub exact_errors: bool,

    /// Is scripting enabled?
    pub scripting_enabled: bool,

    /// Is this an `iframe srcdoc` document?
    pub iframe_srcdoc: bool,

    /// Are we parsing a HTML fragment?
    pub fragment: bool,

    /// Should we drop the DOCTYPE (if any) from the tree?
    pub drop_doctype: bool,
}

impl Default for TreeBuilderOpts {
    fn default() -> TreeBuilderOpts {
        TreeBuilderOpts {
            exact_errors: false,
            scripting_enabled: true,
            iframe_srcdoc: false,
            fragment: false,
            drop_doctype: false,
        }
    }
}

/// The HTML tree builder.
pub struct TreeBuilder<'sink, Handle, Sink> {
    /// Options controlling the behavior of the tree builder.
    opts: TreeBuilderOpts,

    /// Consumer of tree modifications.
    sink: &'sink mut Sink,

    /// Insertion mode.
    mode: InsertionMode,

    /// Original insertion mode, used by Text and InTableText modes.
    orig_mode: Option<InsertionMode>,

    /// Pending table character tokens.
    pending_table_text: Vec<(SplitStatus, String)>,

    /// Quirks mode as set by the parser.
    /// FIXME: can scripts etc. change this?
    quirks_mode: QuirksMode,

    /// The document node, which is created by the sink.
    doc_handle: Handle,

    /// Stack of open elements, most recently added at end.
    open_elems: Vec<Handle>,

    /// List of active formatting elements.
    active_formatting: Vec<FormatEntry<Handle>>,

    //§ the-element-pointers
    /// Head element pointer.
    head_elem: Option<Handle>,

    /// Form element pointer.
    form_elem: Option<Handle>,
    //§ END

    /// Next state change for the tokenizer, if any.
    next_tokenizer_state: Option<tokenizer::states::State>,

    /// Frameset-ok flag.
    frameset_ok: bool,

    /// Ignore a following U+000A LINE FEED?
    ignore_lf: bool,

    /// Is foster parenting enabled?
    foster_parenting: bool,
}

impl<'sink, Handle: Clone, Sink: TreeSink<Handle>> TreeBuilder<'sink, Handle, Sink> {
    /// Create a new tree builder which sends tree modifications to a particular `TreeSink`.
    ///
    /// The tree builder is also a `TokenSink`.
    pub fn new(sink: &'sink mut Sink, opts: TreeBuilderOpts) -> TreeBuilder<'sink, Handle, Sink> {
        let doc_handle = sink.get_document();
        TreeBuilder {
            opts: opts,
            sink: sink,
            mode: Initial,
            orig_mode: None,
            pending_table_text: vec!(),
            quirks_mode: NoQuirks,
            doc_handle: doc_handle,
            open_elems: vec!(),
            active_formatting: vec!(),
            head_elem: None,
            form_elem: None,
            next_tokenizer_state: None,
            frameset_ok: true,
            ignore_lf: false,
            foster_parenting: false,
        }
    }

    // Debug helper
    #[allow(dead_code)]
    fn dump_state(&self, label: String) {
        println!("dump_state on {}", label);
        print!("    open_elems:");
        for node in self.open_elems.iter() {
            let (ns, name) = self.sink.elem_name(node.clone());
            match ns {
                HTML => print!(" {}", name),
                _ => fail!(),
            }
        }
        println!("");
    }

    fn process_to_completion(&mut self, mut token: Token) {
        // Additional tokens yet to be processed. First to be processed is on
        // the *end*, because that's where Vec supports O(1) push/pop.
        // This stays empty (and hence non-allocating) in the common case
        // where we don't split whitespace.
        let mut more_tokens = vec!();

        loop {
            let is_self_closing = match token {
                TagToken(Tag { self_closing: c, .. }) => c,
                _ => false,
            };
            let mode = self.mode;
            match self.step(mode, token) {
                Done => {
                    if is_self_closing {
                        self.sink.parse_error(Slice("Unacknowledged self-closing tag"));
                    }
                    token = unwrap_or_return!(more_tokens.pop(), ());
                }
                DoneAckSelfClosing => {
                    token = unwrap_or_return!(more_tokens.pop(), ());
                }
                Reprocess(m, t) => {
                    self.mode = m;
                    token = t;
                }
                SplitWhitespace(buf) => {
                    let mut it = Runs::new(is_ascii_whitespace, buf.as_slice())
                        .map(|(m, b)| CharacterTokens(match m {
                            true => Whitespace,
                            false => NotWhitespace,
                        }, b.to_string()));

                    token = unwrap_or_return!(it.next(), ());

                    // Push additional tokens in reverse order, so the next one
                    // is first to be popped.
                    // FIXME: copy/allocate less
                    let rest: Vec<Token> = it.collect();
                    for t in rest.move_iter().rev() {
                        more_tokens.push(t);
                    }
                }
            }
        }
    }
}

impl<'sink, Handle: Clone, Sink: TreeSink<Handle>> TokenSink for TreeBuilder<'sink, Handle, Sink> {
    fn process_token(&mut self, token: tokenizer::Token) {
        let ignore_lf = replace(&mut self.ignore_lf, false);

        // Handle `ParseError` and `DoctypeToken`; convert everything else to the local `Token` type.
        let token = match token {
            tokenizer::ParseError(e) => {
                self.sink.parse_error(e);
                return;
            }

            tokenizer::DoctypeToken(dt) => if self.mode == Initial {
                let (err, quirk) = data::doctype_error_and_quirks(&dt, self.opts.iframe_srcdoc);
                if err {
                    self.sink.parse_error(format_if!(
                        self.opts.exact_errors,
                        "Bad DOCTYPE",
                        "Bad DOCTYPE: {}", dt));
                }
                let Doctype { name, public_id, system_id, force_quirks: _ } = dt;
                if !self.opts.drop_doctype {
                    self.sink.append_doctype_to_document(
                        name.unwrap_or(String::new()),
                        public_id.unwrap_or(String::new()),
                        system_id.unwrap_or(String::new())
                    );
                }
                self.set_quirks_mode(quirk);

                self.mode = BeforeHtml;
                return;
            } else {
                self.sink.parse_error(format_if!(
                    self.opts.exact_errors,
                    "DOCTYPE in body",
                    "DOCTYPE in insertion mode {:?}", self.mode));
                return;
            },

            tokenizer::TagToken(x) => TagToken(x),
            tokenizer::CommentToken(x) => CommentToken(x),
            tokenizer::NullCharacterToken => NullCharacterToken,
            tokenizer::EOFToken => EOFToken,

            tokenizer::CharacterTokens(mut x) => {
                if ignore_lf && x.len() >= 1 && x.as_slice().char_at(0) == '\n' {
                    x.shift_char();
                }
                if x.is_empty() {
                    return;
                }
                CharacterTokens(NotSplit, x)
            }
        };

        self.process_to_completion(token);
    }

    fn query_state_change(&mut self) -> Option<tokenizer::states::State> {
        self.next_tokenizer_state.take()
    }
}
