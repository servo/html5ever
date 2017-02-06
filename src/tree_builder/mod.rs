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
pub use self::interface::{TreeSink, Tracer};

use self::types::*;
use self::actions::TreeBuilderActions;
use self::rules::TreeBuilderStep;

use QualName;
use tendril::StrTendril;

use tokenizer;
use tokenizer::{Doctype, StartTag, Tag, TokenSink, TokenSinkResult};
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

mod rules {
    //! The tree builder rules, as a single, enormous nested match expression.

    include!(concat!(env!("OUT_DIR"), "/rules.rs"));
}

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

    /// Frameset-ok flag.
    frameset_ok: bool,

    /// Ignore a following U+000A LINE FEED?
    ignore_lf: bool,

    /// Is foster parenting enabled?
    foster_parenting: bool,

    /// The context element for the fragment parsing algorithm.
    context_elem: Option<Handle>,

    /// Track current line
    current_line: u64,

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
            frameset_ok: true,
            ignore_lf: false,
            foster_parenting: false,
            context_elem: None,
            current_line: 1,
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
            frameset_ok: true,
            ignore_lf: false,
            foster_parenting: false,
            context_elem: Some(context_elem),
            current_line: 1,
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
            local_name!("title") | local_name!("textarea") => tok_state::RawData(tok_state::Rcdata),

            local_name!("style") | local_name!("xmp") | local_name!("iframe")
                | local_name!("noembed") | local_name!("noframes") => tok_state::RawData(tok_state::Rawtext),

            local_name!("script") => tok_state::RawData(tok_state::ScriptData),

            local_name!("noscript") => if self.opts.scripting_enabled {
                tok_state::RawData(tok_state::Rawtext)
            } else {
                tok_state::Data
            },

            local_name!("plaintext") => tok_state::Plaintext,

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

    fn process_to_completion(&mut self, mut token: Token) -> TokenSinkResult<Handle> {
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
                    token = unwrap_or_return!(more_tokens.pop_front(), tokenizer::TokenSinkResult::Continue);
                }
                DoneAckSelfClosing => {
                    token = unwrap_or_return!(more_tokens.pop_front(), tokenizer::TokenSinkResult::Continue);
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
                    let (first, is_ws) = unwrap_or_return!(p, tokenizer::TokenSinkResult::Continue);
                    let status = if is_ws { Whitespace } else { NotWhitespace };
                    token = CharacterTokens(status, first);

                    if buf.len32() > 0 {
                        more_tokens.push_back(CharacterTokens(NotSplit, buf));
                    }
                }
                Script(node) => {
                    assert!(more_tokens.is_empty());
                    return tokenizer::TokenSinkResult::Script(node);
                }
                ToPlaintext => {
                    assert!(more_tokens.is_empty());
                    return tokenizer::TokenSinkResult::Plaintext;
                }
                ToRawData(k) => {
                    assert!(more_tokens.is_empty());
                    return tokenizer::TokenSinkResult::RawData(k);
                }
            }
        }
    }

    /// Are we parsing a HTML fragment?
    pub fn is_fragment(&self) -> bool {
        self.context_elem.is_some()
    }

    fn appropriate_place_for_insertion(&mut self,
                                       override_target: Option<Handle>)
                                       -> InsertionPoint<Handle> {
        use self::tag_sets::*;

        declare_tag_set!(foster_target = "table" "tbody" "tfoot" "thead" "tr");
        let target = override_target.unwrap_or_else(|| self.current_node());
        if !(self.foster_parenting && self.elem_in(target.clone(), foster_target)) {
            if self.html_elem_named(target.clone(), local_name!("template")) {
                // No foster parenting (inside template).
                let contents = self.sink.get_template_contents(target);
                return LastChild(contents);
            } else {
                // No foster parenting (the common case).
                return LastChild(target);
            }
        }

        // Foster parenting
        let mut iter = self.open_elems.iter().rev().peekable();
        while let Some(elem) = iter.next() {
            if self.html_elem_named(elem.clone(), local_name!("template")) {
                let contents = self.sink.get_template_contents(elem.clone());
                return LastChild(contents);
            } else if self.html_elem_named(elem.clone(), local_name!("table")) {
                // Try inserting "inside last table's parent node, immediately before last table"
                if self.sink.has_parent_node(elem.clone()) {
                    return BeforeSibling(elem.clone());
                } else {
                    // If elem has no parent, we regain ownership of the child.
                    // Insert "inside previous element, after its last child (if any)"
                    let previous_element = (*iter.peek().unwrap()).clone();
                    return LastChild(previous_element);
                }
            }
        }
        let html_elem = self.html_elem();
        LastChild(html_elem)
    }

    fn insert_at(&mut self, insertion_point: InsertionPoint<Handle>, child: NodeOrText<Handle>) {
        match insertion_point {
            LastChild(parent) => self.sink.append(parent, child),
            BeforeSibling(sibling) => self.sink.append_before_sibling(sibling, child)
        }
    }
}

impl<Handle, Sink> TokenSink
    for TreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{
    type Handle = Handle;

    fn process_token(&mut self, token: tokenizer::Token, line_number: u64) -> TokenSinkResult<Handle> {
        if line_number != self.current_line {
            self.sink.set_current_line(line_number);
        }
        let ignore_lf = replace(&mut self.ignore_lf, false);

        // Handle `ParseError` and `DoctypeToken`; convert everything else to the local `Token` type.
        let token = match token {
            tokenizer::ParseError(e) => {
                self.sink.parse_error(e);
                return tokenizer::TokenSinkResult::Continue;
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
                return tokenizer::TokenSinkResult::Continue;
            } else {
                self.sink.parse_error(format_if!(
                    self.opts.exact_errors,
                    "DOCTYPE in body",
                    "DOCTYPE in insertion mode {:?}", self.mode));
                return tokenizer::TokenSinkResult::Continue;
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
                    return tokenizer::TokenSinkResult::Continue;
                }
                CharacterTokens(NotSplit, x)
            }
        };

        self.process_to_completion(token)
    }

    fn end(&mut self) {
        for elem in self.open_elems.drain(..).rev() {
            self.sink.pop(elem);
        }
    }

    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        !self.open_elems.is_empty() &&
        self.sink.elem_name(self.adjusted_current_node()).ns != ns!(html)
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use super::interface::{QuirksMode, Quirks, LimitedQuirks, NoQuirks};
    use super::interface::{NodeOrText, AppendNode, AppendText};
    use super::interface::{TreeSink, Tracer};

    use super::types::*;
    use super::actions::TreeBuilderActions;
    use super::rules::TreeBuilderStep;

    use QualName;
    use tendril::StrTendril;
    use tendril::stream::{TendrilSink, Utf8LossyDecoder, LossyDecoder};

    use tokenizer;
    use tokenizer::{Tokenizer, TokenizerOpts};
    use tokenizer::{Doctype, StartTag, Tag, TokenSink};
    use tokenizer::states as tok_state;

    use util::str::is_ascii_whitespace;

    use std::default::Default;
    use std::mem::replace;
    use std::borrow::Cow;
    use std::borrow::Cow::Borrowed;
    use std::collections::VecDeque;

    use driver::*;
    use super::{TreeBuilderOpts, TreeBuilder};
    use tokenizer::Attribute;
    use rcdom::{Node, Handle, RcDom, NodeEnum, ElementEnum};

    pub struct LineCountingDOM {
        pub line_vec: Vec<(QualName, u64)>,
        pub current_line: u64,
        pub rcdom: RcDom,
    }

    impl TreeSink for LineCountingDOM {
        type Output = Self;

        fn finish(self) -> Self { self }

        type Handle = Handle;

        fn parse_error(&mut self, msg: Cow<'static, str>) {
            self.rcdom.parse_error(msg);
        }

        fn get_document(&mut self) -> Handle {
            self.rcdom.get_document()
        }

        fn get_template_contents(&mut self, target: Handle) -> Handle {
            self.rcdom.get_template_contents(target)
        }

        fn set_quirks_mode(&mut self, mode: QuirksMode) {
            self.rcdom.set_quirks_mode(mode)
        }

        fn same_node(&self, x: Handle, y: Handle) -> bool {
            self.rcdom.same_node(x, y)
        }

        fn elem_name(&self, target: Handle) -> QualName {
            self.rcdom.elem_name(target)
        }

        fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>) -> Handle {
            self.line_vec.push((name.clone(), self.current_line));
            self.rcdom.create_element(name, attrs)
        }

        fn create_comment(&mut self, text: StrTendril) -> Handle {
            self.rcdom.create_comment(text)
        }

        fn has_parent_node(&self, node: Handle) -> bool {
            let node = node.borrow();
            node.parent.is_some()
        }

        fn append(&mut self, parent: Handle, child: NodeOrText<Handle>) {
            self.rcdom.append(parent, child)
        }

        fn append_before_sibling(&mut self,
                sibling: Handle,
                child: NodeOrText<Handle>) {
            self.rcdom.append_before_sibling(sibling, child)
        }

        fn append_doctype_to_document(&mut self,
                                      name: StrTendril,
                                      public_id: StrTendril,
                                      system_id: StrTendril) {
            self.rcdom.append_doctype_to_document(name, public_id, system_id);
        }

        fn add_attrs_if_missing(&mut self, target: Handle, attrs: Vec<Attribute>) {
            self.rcdom.add_attrs_if_missing(target, attrs);
        }

        fn remove_from_parent(&mut self, target: Handle) {
            self.rcdom.remove_from_parent(target);
        }

        fn reparent_children(&mut self, node: Handle, new_parent: Handle) {
            self.rcdom.reparent_children(node, new_parent);
        }

        fn mark_script_already_started(&mut self, target: Handle) {
            self.rcdom.mark_script_already_started(target);
        }

        fn is_mathml_annotation_xml_integration_point(&self, handle: Self::Handle) -> bool {
            self.rcdom.is_mathml_annotation_xml_integration_point(handle)
        }

        fn set_current_line(&mut self, line_number: u64) {
            self.current_line = line_number;
        }
    }

    #[test]
    fn check_four_lines() {
        // Input
        let sink = LineCountingDOM {
                line_vec: vec!(),
                current_line: 1,
                rcdom: RcDom::default(),
            };
        let opts = ParseOpts::default();
        let mut resultTok = parse_document(sink, opts);
        resultTok.process(StrTendril::from("<a>\n"));
        resultTok.process(StrTendril::from("</a>\n"));
        resultTok.process(StrTendril::from("<b>\n"));
        resultTok.process(StrTendril::from("</b>"));
        // Actual Output
        let actual = resultTok.finish();
        // Expected Output
        let expected = vec![(qualname!(html, "html"), 1),
                            (qualname!(html, "head"), 1),
                            (qualname!(html, "body"), 1),
                            (qualname!(html, "a"), 1),
                            (qualname!(html, "b"), 3)];
        // Assertion
        assert_eq!(actual.line_vec, expected);
    }
}
