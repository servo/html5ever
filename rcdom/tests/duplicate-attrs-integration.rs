// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use html5ever::driver;
use html5ever::tendril::stream::TendrilSink;
use html5ever::tendril::StrTendril;
use html5ever::ExpandedName;
use html5ever::QualName;
use markup5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use markup5ever::{local_name, ns, Attribute};
use markup5ever_rcdom::{Handle, RcDom};
use std::borrow::Cow;
use std::cell::RefCell;

/// A TreeSink that captures had_duplicate_attrs flags for created elements
pub struct FlagCapturingDOM {
    pub had_duplicate_attrs_flags: RefCell<Vec<bool>>,
    pub rcdom: RcDom,
}
impl Default for FlagCapturingDOM {
    fn default() -> Self {
        Self::new()
    }
}

impl FlagCapturingDOM {
    pub fn new() -> Self {
        FlagCapturingDOM {
            had_duplicate_attrs_flags: RefCell::new(Vec::new()),
            rcdom: RcDom::default(),
        }
    }
}

impl TreeSink for FlagCapturingDOM {
    type Output = Self;
    type ElemName<'a> = ExpandedName<'a>;

    fn finish(self) -> Self {
        self
    }

    type Handle = Handle;

    fn parse_error(&self, msg: Cow<'static, str>) {
        self.rcdom.parse_error(msg);
    }

    fn get_document(&self) -> Handle {
        self.rcdom.get_document()
    }

    fn get_template_contents(&self, target: &Handle) -> Handle {
        self.rcdom.get_template_contents(target)
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.rcdom.set_quirks_mode(mode)
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        self.rcdom.same_node(x, y)
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> ExpandedName<'a> {
        self.rcdom.elem_name(target)
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        // Capture the had_duplicate_attrs flag value for inspection
        self.had_duplicate_attrs_flags
            .borrow_mut()
            .push(flags.had_duplicate_attrs);
        self.rcdom.create_element(name, attrs, flags)
    }

    fn create_comment(&self, text: StrTendril) -> Handle {
        self.rcdom.create_comment(text)
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Handle {
        self.rcdom.create_pi(target, data)
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        self.rcdom.append(parent, child)
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        self.rcdom
            .append_based_on_parent_node(element, prev_element, child)
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        self.rcdom
            .append_doctype_to_document(name, public_id, system_id)
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        self.rcdom.add_attrs_if_missing(target, attrs)
    }

    fn remove_from_parent(&self, target: &Handle) {
        self.rcdom.remove_from_parent(target)
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        self.rcdom.reparent_children(node, new_parent)
    }

    fn mark_script_already_started(&self, node: &Handle) {
        self.rcdom.mark_script_already_started(node)
    }

    fn set_current_line(&self, line_number: u64) {
        self.rcdom.set_current_line(line_number)
    }

    fn pop(&self, node: &Handle) {
        self.rcdom.pop(node)
    }

    fn append_before_sibling(&self, sibling: &Handle, new_node: NodeOrText<Handle>) {
        self.rcdom.append_before_sibling(sibling, new_node)
    }
}

#[test]
fn test_duplicate_attrs_flag_set() {
    let sink = FlagCapturingDOM::new();
    let input = r#"<div id="first" id="second"></div>"#;

    let sink = driver::parse_fragment(
        sink,
        driver::ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("body")),
        vec![],
        false, // context_element_allows_scripting
    )
    .one(input)
    .finish();

    // We expect at least one element to have been created (the div)
    // Find if any element has had_duplicate_attrs=true
    let flags = sink.had_duplicate_attrs_flags.borrow();
    let has_duplicate = flags.iter().any(|&f| f);

    assert!(
        has_duplicate,
        "Expected to find an element with had_duplicate_attrs=true"
    );
}

#[test]
fn test_no_duplicate_attrs_flag_not_set() {
    let sink = FlagCapturingDOM::new();
    let input = r#"<div id="first" class="test"></div>"#;

    let sink = driver::parse_fragment(
        sink,
        driver::ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("body")),
        vec![],
        false,
    )
    .one(input)
    .finish();

    // All flags should have had_duplicate_attrs=false
    let flags = sink.had_duplicate_attrs_flags.borrow();
    for flag in flags.iter() {
        assert!(
            !flag,
            "Expected had_duplicate_attrs to be false for elements without duplicates"
        );
    }
}

#[test]
fn test_multiple_duplicates_sets_flag() {
    let sink = FlagCapturingDOM::new();
    let input = r#"<div id="a" id="b" class="x" class="y"></div>"#;

    let sink = driver::parse_fragment(
        sink,
        driver::ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("body")),
        vec![],
        false,
    )
    .one(input)
    .finish();

    let flags = sink.had_duplicate_attrs_flags.borrow();
    let has_duplicate = flags.iter().any(|&f| f);

    assert!(
        has_duplicate,
        "Expected to find an element with had_duplicate_attrs=true for multiple duplicates"
    );
}

#[test]
fn test_script_with_duplicate_nonce() {
    let sink = FlagCapturingDOM::new();
    let input = r#"<script nonce="abc" nonce="xyz"></script>"#;

    let sink = driver::parse_fragment(
        sink,
        driver::ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("body")),
        vec![],
        false,
    )
    .one(input)
    .finish();

    let flags = sink.had_duplicate_attrs_flags.borrow();
    let has_duplicate = flags.iter().any(|&f| f);

    assert!(
        has_duplicate,
        "Expected script with duplicate nonce to have had_duplicate_attrs=true"
    );
}
