// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(warnings)]

//! The HTML5 tree builder.

pub use self::interface::{QuirksMode, Quirks, LimitedQuirks, NoQuirks};
pub use self::interface::{NodeOrText, AppendNode, AppendText};
pub use self::interface::{TreeSink, Tracer, NextParserState};

use self::types::*;
use self::actions::TreeBuilderActions;
use self::rules::TreeBuilderStep;

use string_cache::QualName;
use tendril::StrTendril;

use tokenizer;
use tokenizer::{Doctype, StartTag, Tag, TokenSink};
use tokenizer::states as tok_state;

use util::str::is_ascii_whitespace;

use std::default::Default;
use std::mem::replace;
use std::borrow::Cow::Borrowed;
use std::collections::VecDeque;

#[macro_use] mod tag_sets;
// "pub" is a workaround for rust#18241 (?)
pub mod interface;
mod data;
mod types;
mod actions;
#[path = "rules.expanded.rs"] mod rules;

/// Tree builder options, with an impl for Default.
#[derive(Copy, Clone)]
pub struct TreeBuilderOpts {
    /// Report all parse errors described in the spec, at some
    /// performance penalty?  Default: false
    pub exact_errors: bool,

    /// Is scripting enabled?
    pub scripting_enabled: bool,

    /// Is this an `iframe srcdoc` document?
    pub iframe_srcdoc: bool,

    /// Should we drop the DOCTYPE (if any) from the tree?
    pub drop_doctype: bool,

    /// Obsolete, ignored.
    pub ignore_missing_rules: bool,

    /// Initial TreeBuilder quirks mode. Default: NoQuirks
    pub quirks_mode: QuirksMode,
}

impl Default for TreeBuilderOpts {
    fn default() -> TreeBuilderOpts {
        TreeBuilderOpts {
            exact_errors: false,
            scripting_enabled: true,
            iframe_srcdoc: false,
            drop_doctype: false,
            ignore_missing_rules: false,
            quirks_mode: NoQuirks,
        }
    }
}

/// The HTML tree builder.
pub struct TreeBuilder<Handle, Sink> {
    /// Options controlling the behavior of the tree builder.
    opts: TreeBuilderOpts,

    /// Consumer of tree modifications.
    sink: Sink,

    /// Insertion mode.
    mode: InsertionMode,

    /// Original insertion mode, used by Text and InTableText modes.
    orig_mode: Option<InsertionMode>,

    /// Stack of template insertion modes.
    template_modes: Vec<InsertionMode>,

    /// Pending table character tokens.
    pending_table_text: Vec<(SplitStatus, StrTendril)>,

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

    /// The context element for the fragment parsing algorithm.
    context_elem: Option<Handle>,

    // WARNING: If you add new fields that contain Handles, you
    // must add them to trace_handles() below to preserve memory
    // safety!
    //
    // FIXME: Auto-generate the trace hooks like Servo does.
}

impl<Handle, Sink> TreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{
    /// Create a new tree builder which sends tree modifications to a particular `TreeSink`.
    ///
    /// The tree builder is also a `TokenSink`.
    pub fn new(mut sink: Sink, opts: TreeBuilderOpts) -> TreeBuilder<Handle, Sink> {
        let doc_handle = sink.get_document();
        TreeBuilder {
            opts: opts,
            sink: sink,
            mode: Initial,
            orig_mode: None,
            template_modes: vec!(),
            pending_table_text: vec!(),
            quirks_mode: opts.quirks_mode,
            doc_handle: doc_handle,
            open_elems: vec!(),
            active_formatting: vec!(),
            head_elem: None,
            form_elem: None,
            next_tokenizer_state: None,
            frameset_ok: true,
            ignore_lf: false,
            foster_parenting: false,
            context_elem: None,
        }
    }

    /// Create a new tree builder which sends tree modifications to a particular `TreeSink`.
    /// This is for parsing fragments.
    ///
    /// The tree builder is also a `TokenSink`.
    pub fn new_for_fragment(mut sink: Sink,
                            context_elem: Handle,
                            form_elem: Option<Handle>,
                            opts: TreeBuilderOpts) -> TreeBuilder<Handle, Sink> {
        let doc_handle = sink.get_document();
        let context_is_template =
            sink.elem_name(context_elem.clone()) == qualname!(html, "template");
        let mut tb = TreeBuilder {
            opts: opts,
            sink: sink,
            mode: Initial,
            orig_mode: None,
            template_modes: if context_is_template { vec![InTemplate] } else { vec![] },
            pending_table_text: vec!(),
            quirks_mode: opts.quirks_mode,
            doc_handle: doc_handle,
            open_elems: vec!(),
            active_formatting: vec!(),
            head_elem: None,
            form_elem: form_elem,
            next_tokenizer_state: None,
            frameset_ok: true,
            ignore_lf: false,
            foster_parenting: false,
            context_elem: Some(context_elem),
        };

        // https://html.spec.whatwg.org/multipage/syntax.html#parsing-html-fragments
        // 5. Let root be a new html element with no attributes.
        // 6. Append the element root to the Document node created above.
        // 7. Set up the parser's stack of open elements so that it contains just the single element root.
        tb.create_root(vec!());
        // 10. Reset the parser's insertion mode appropriately.
        tb.mode = tb.reset_insertion_mode();

        tb
    }

    // https://html.spec.whatwg.org/multipage/syntax.html#concept-frag-parse-context
    // Step 4. Set the state of the HTML parser's tokenization stage as follows:
    pub fn tokenizer_state_for_context_elem(&self) -> tok_state::State {
        let elem = self.context_elem.clone().expect("no context element");
        let name = match self.sink.elem_name(elem) {
            QualName { ns: ns!(html), local } => local,
            _ => return tok_state::Data
        };
        match name {
            atom!("title") | atom!("textarea") => tok_state::RawData(tok_state::Rcdata),

            atom!("style") | atom!("xmp") | atom!("iframe")
                | atom!("noembed") | atom!("noframes") => tok_state::RawData(tok_state::Rawtext),

            atom!("script") => tok_state::RawData(tok_state::ScriptData),

            atom!("noscript") => if self.opts.scripting_enabled {
                tok_state::RawData(tok_state::Rawtext)
            } else {
                tok_state::Data
            },

            atom!("plaintext") => tok_state::Plaintext,

            _ => tok_state::Data
        }
    }

    pub fn unwrap(self) -> Sink {
        self.sink
    }

    pub fn sink<'a>(&'a self) -> &'a Sink {
        &self.sink
    }

    pub fn sink_mut<'a>(&'a mut self) -> &'a mut Sink {
        &mut self.sink
    }

    /// Call the `Tracer`'s `trace_handle` method on every `Handle` in the tree builder's
    /// internal state.  This is intended to support garbage-collected DOMs.
    pub fn trace_handles(&self, tracer: &Tracer<Handle=Handle>) {
        tracer.trace_handle(&self.doc_handle);
        for e in &self.open_elems {
            tracer.trace_handle(e);
        }
        for e in &self.active_formatting {
            match e {
                &Element(ref h, _) => tracer.trace_handle(h),
                _ => (),
            }
        }
        self.head_elem.as_ref().map(|h| tracer.trace_handle(h));
        self.form_elem.as_ref().map(|h| tracer.trace_handle(h));
        self.context_elem.as_ref().map(|h| tracer.trace_handle(h));
    }

    #[allow(dead_code)]
    fn dump_state(&self, label: String) {
        use string_cache::QualName;

        println!("dump_state on {}", label);
        print!("    open_elems:");
        for node in self.open_elems.iter() {
            let QualName { ns, local } = self.sink.elem_name(node.clone());
            match ns {
                ns!(html) => print!(" {}", &local[..]),
                _ => panic!(),
            }
        }
        println!("");
        print!("    active_formatting:");
        for entry in self.active_formatting.iter() {
            match entry {
                &Marker => print!(" Marker"),
                &Element(ref h, _) => {
                    let QualName { ns, local } = self.sink.elem_name(h.clone());
                    match ns {
                        ns!(html) => print!(" {}", &local[..]),
                        _ => panic!(),
                    }
                }
            }
        }
        println!("");
    }

    fn debug_step(&self, mode: InsertionMode, token: &Token) {
        use util::str::to_escaped_string;
        debug!("processing {} in insertion mode {:?}", to_escaped_string(token), mode);
    }

    fn process_to_completion(&mut self, mut token: Token) {
        // Queue of additional tokens yet to be processed.
        // This stays empty in the common case where we don't split whitespace.
        let mut more_tokens = VecDeque::new();

        loop {
            let should_have_acknowledged_self_closing_flag =
                matches!(token, TagToken(Tag { self_closing: true, kind: StartTag, .. }));
            let result = if self.is_foreign(&token) {
                self.step_foreign(token)
            } else {
                let mode = self.mode;
                self.step(mode, token)
            };
            match result {
                Done => {
                    if should_have_acknowledged_self_closing_flag {
                        self.sink.parse_error(Borrowed("Unacknowledged self-closing tag"));
                    }
                    token = unwrap_or_return!(more_tokens.pop_front(), ());
                }
                DoneAckSelfClosing => {
                    token = unwrap_or_return!(more_tokens.pop_front(), ());
                }
                Reprocess(m, t) => {
                    self.mode = m;
                    token = t;
                }
                ReprocessForeign(t) => {
                    token = t;
                }
                SplitWhitespace(mut buf) => {
                    let p = buf.pop_front_char_run(is_ascii_whitespace);
                    let (first, is_ws) = unwrap_or_return!(p, ());
                    let status = if is_ws { Whitespace } else { NotWhitespace };
                    token = CharacterTokens(status, first);

                    if buf.len32() > 0 {
                        more_tokens.push_back(CharacterTokens(NotSplit, buf));
                    }
                }
            }
        }
    }

    /// Are we parsing a HTML fragment?
    pub fn is_fragment(&self) -> bool {
        self.context_elem.is_some()
    }
}

impl<Handle, Sink> TokenSink
    for TreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{
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
                        "Bad DOCTYPE: {:?}", dt));
                }
                let Doctype { name, public_id, system_id, force_quirks: _ } = dt;
                if !self.opts.drop_doctype {
                    self.sink.append_doctype_to_document(
                        name.unwrap_or(StrTendril::new()),
                        public_id.unwrap_or(StrTendril::new()),
                        system_id.unwrap_or(StrTendril::new())
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
                if ignore_lf && x.starts_with("\n") {
                    x.pop_front(1);
                }
                if x.is_empty() {
                    return;
                }
                CharacterTokens(NotSplit, x)
            }
        };

        self.process_to_completion(token);
    }

    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        !self.open_elems.is_empty() &&
        self.sink.elem_name(self.adjusted_current_node()).ns != ns!(html)
    }

    fn query_state_change(&mut self) -> Option<tokenizer::states::State> {
        self.next_tokenizer_state.take()
    }
}
