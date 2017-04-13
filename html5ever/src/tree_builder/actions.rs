// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Helpers for implementing the tree builder rules.
//!
//! Many of these are named within the spec, e.g. "reset the insertion
//! mode appropriately".

use tree_builder::types::*;
use tree_builder::tag_sets::*;
use tree_builder::interface::{TreeSink, QuirksMode, NodeOrText, AppendNode, AppendText};
use tree_builder::rules::TreeBuilderStep;

use tokenizer::{Attribute, Tag, StartTag, EndTag};
use tokenizer::states::{RawData, RawKind};

use util::str::to_escaped_string;

use std::ascii::AsciiExt;
use std::{slice, fmt};
use std::mem::replace;
use std::iter::{Rev, Enumerate};
use std::borrow::Cow::Borrowed;

use {LocalName, Namespace, QualName};
use tendril::StrTendril;

pub use self::PushFlag::*;

pub struct ActiveFormattingIter<'a, Handle: 'a> {
    iter: Rev<Enumerate<slice::Iter<'a, FormatEntry<Handle>>>>,
}

impl<'a, Handle> Iterator for ActiveFormattingIter<'a, Handle> {
    type Item = (usize, &'a Handle, &'a Tag);
    fn next(&mut self) -> Option<(usize, &'a Handle, &'a Tag)> {
        match self.iter.next() {
            None | Some((_, &Marker)) => None,
            Some((i, &Element(ref h, ref t))) => Some((i, h, t)),
        }
    }
}

pub enum PushFlag {
    Push,
    NoPush,
}

enum Bookmark<Handle> {
    Replace(Handle),
    InsertAfter(Handle),
}

// These go in a trait so that we can control visibility.
pub trait TreeBuilderActions<Handle> {
    fn unexpected<T: fmt::Debug>(&mut self, thing: &T) -> ProcessResult<Handle>;
    fn assert_named(&mut self, node: Handle, name: LocalName);
    fn clear_active_formatting_to_marker(&mut self);
    fn create_formatting_element_for(&mut self, tag: Tag) -> Handle;
    fn append_text(&mut self, text: StrTendril) -> ProcessResult<Handle>;
    fn append_comment(&mut self, text: StrTendril) -> ProcessResult<Handle>;
    fn append_comment_to_doc(&mut self, text: StrTendril) -> ProcessResult<Handle>;
    fn append_comment_to_html(&mut self, text: StrTendril) -> ProcessResult<Handle>;
    fn insert_appropriately(&mut self, child: NodeOrText<Handle>, override_target: Option<Handle>);
    fn insert_phantom(&mut self, name: LocalName) -> Handle;
    fn insert_and_pop_element_for(&mut self, tag: Tag) -> Handle;
    fn insert_element_for(&mut self, tag: Tag) -> Handle;
    fn insert_element(&mut self, push: PushFlag, ns: Namespace, name: LocalName, attrs: Vec<Attribute>) -> Handle;
    fn create_root(&mut self, attrs: Vec<Attribute>);
    fn close_the_cell(&mut self);
    fn reset_insertion_mode(&mut self) -> InsertionMode;
    fn process_chars_in_table(&mut self, token: Token) -> ProcessResult<Handle>;
    fn foster_parent_in_body(&mut self, token: Token) -> ProcessResult<Handle>;
    fn is_type_hidden(&self, tag: &Tag) -> bool;
    fn close_p_element_in_button_scope(&mut self);
    fn close_p_element(&mut self);
    fn expect_to_close(&mut self, name: LocalName);
    fn pop_until_named(&mut self, name: LocalName) -> usize;
    fn pop_until<TagSet>(&mut self, pred: TagSet) -> usize where TagSet: Fn(QualName) -> bool;
    fn pop_until_current<TagSet>(&mut self, pred: TagSet) where TagSet: Fn(QualName) -> bool;
    fn generate_implied_end_except(&mut self, except: LocalName);
    fn generate_implied_end<TagSet>(&mut self, set: TagSet) where TagSet: Fn(QualName) -> bool;
    fn in_scope_named<TagSet>(&self, scope: TagSet, name: LocalName) -> bool where TagSet: Fn(QualName) -> bool;
    fn current_node_named(&self, name: LocalName) -> bool;
    fn html_elem_named(&self, elem: Handle, name: LocalName) -> bool;
    fn in_html_elem_named(&self, name: LocalName) -> bool;
    fn elem_in<TagSet>(&self, elem: Handle, set: TagSet) -> bool where TagSet: Fn(QualName) -> bool;
    fn in_scope<TagSet,Pred>(&self, scope: TagSet, pred: Pred) -> bool where TagSet: Fn(QualName) -> bool, Pred: Fn(Handle) -> bool;
    fn check_body_end(&mut self);
    fn body_elem(&mut self) -> Option<Handle>;
    fn html_elem(&self) -> Handle;
    fn reconstruct_formatting(&mut self);
    fn remove_from_stack(&mut self, elem: &Handle);
    fn pop(&mut self) -> Handle;
    fn push(&mut self, elem: &Handle);
    fn adoption_agency(&mut self, subject: LocalName);
    fn current_node_in<TagSet>(&self, set: TagSet) -> bool where TagSet: Fn(QualName) -> bool;
    fn current_node(&self) -> Handle;
    fn adjusted_current_node(&self) -> Handle;
    fn parse_raw_data(&mut self, tag: Tag, k: RawKind) -> ProcessResult<Handle>;
    fn to_raw_text_mode(&mut self, k: RawKind) -> ProcessResult<Handle>;
    fn stop_parsing(&mut self) -> ProcessResult<Handle>;
    fn set_quirks_mode(&mut self, mode: QuirksMode);
    fn active_formatting_end_to_marker<'a>(&'a self) -> ActiveFormattingIter<'a, Handle>;
    fn is_marker_or_open(&self, entry: &FormatEntry<Handle>) -> bool;
    fn position_in_active_formatting(&self, element: &Handle) -> Option<usize>;
    fn process_end_tag_in_body(&mut self, tag: Tag);
    fn handle_misnested_a_tags(&mut self, tag: &Tag);
    fn is_foreign(&mut self, token: &Token) -> bool;
    fn enter_foreign(&mut self, tag: Tag, ns: Namespace) -> ProcessResult<Handle>;
    fn adjust_attributes<F>(&mut self, tag: &mut Tag, mut map: F)
        where F: FnMut(LocalName) -> Option<QualName>;
    fn adjust_svg_tag_name(&mut self, tag: &mut Tag);
    fn adjust_svg_attributes(&mut self, tag: &mut Tag);
    fn adjust_mathml_attributes(&mut self, tag: &mut Tag);
    fn adjust_foreign_attributes(&mut self, tag: &mut Tag);
    fn foreign_start_tag(&mut self, tag: Tag) -> ProcessResult<Handle>;
    fn unexpected_start_tag_in_foreign_content(&mut self, tag: Tag) -> ProcessResult<Handle>;
}

#[doc(hidden)]
impl<Handle, Sink> TreeBuilderActions<Handle>
    for super::TreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{
    fn unexpected<T: fmt::Debug>(&mut self, _thing: &T) -> ProcessResult<Handle> {
        self.sink.parse_error(format_if!(
            self.opts.exact_errors,
            "Unexpected token",
            "Unexpected token {} in insertion mode {:?}", to_escaped_string(_thing), self.mode));
        Done
    }

    fn assert_named(&mut self, node: Handle, name: LocalName) {
        assert!(self.html_elem_named(node, name));
    }

    /// Iterate over the active formatting elements (with index in the list) from the end
    /// to the last marker, or the beginning if there are no markers.
    fn active_formatting_end_to_marker<'a>(&'a self) -> ActiveFormattingIter<'a, Handle> {
        ActiveFormattingIter {
            iter: self.active_formatting.iter().enumerate().rev(),
        }
    }

    fn position_in_active_formatting(&self, element: &Handle) -> Option<usize> {
        self.active_formatting
            .iter()
            .position(|n| {
                match n {
                    &Marker => false,
                    &Element(ref handle, _) => self.sink.same_node(handle.clone(), element.clone())
                }
            })
    }

    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        self.quirks_mode = mode;
        self.sink.set_quirks_mode(mode);
    }

    fn stop_parsing(&mut self) -> ProcessResult<Handle> {
        warn!("stop_parsing not implemented, full speed ahead!");
        Done
    }

    //§ parsing-elements-that-contain-only-text
    // Switch to `Text` insertion mode, save the old mode, and
    // switch the tokenizer to a raw-data state.
    // The latter only takes effect after the current / next
    // `process_token` of a start tag returns!
    fn to_raw_text_mode(&mut self, k: RawKind) -> ProcessResult<Handle> {
        self.orig_mode = Some(self.mode);
        self.mode = Text;
        ToRawData(k)
    }

    // The generic raw text / RCDATA parsing algorithm.
    fn parse_raw_data(&mut self, tag: Tag, k: RawKind) -> ProcessResult<Handle> {
        self.insert_element_for(tag);
        self.to_raw_text_mode(k)
    }
    //§ END

    fn current_node(&self) -> Handle {
        self.open_elems.last().expect("no current element").clone()
    }

    fn adjusted_current_node(&self) -> Handle {
        if self.open_elems.len() == 1 {
            if let Some(ctx) = self.context_elem.as_ref() {
                return ctx.clone();
            }
        }
        self.current_node()
    }

    fn current_node_in<TagSet>(&self, set: TagSet) -> bool
        where TagSet: Fn(QualName) -> bool
    {
        set(self.sink.elem_name(self.current_node()))
    }

    // Insert at the "appropriate place for inserting a node".
    fn insert_appropriately(&mut self, child: NodeOrText<Handle>, override_target: Option<Handle>) {
        let insertion_point = self.appropriate_place_for_insertion(override_target);
        self.insert_at(insertion_point, child);
    }

    fn adoption_agency(&mut self, subject: LocalName) {
        // 1.
        if self.current_node_named(subject.clone()) {
            if self.position_in_active_formatting(&self.current_node()).is_none() {
                self.pop();
                return;
            }
        }

        // 2. 3. 4.
        for _ in 0..8 {
            // 5.
            let (fmt_elem_index, fmt_elem, fmt_elem_tag) = unwrap_or_return!(
                // We clone the Handle and Tag so they don't cause an immutable borrow of self.
                self.active_formatting_end_to_marker()
                    .filter(|&(_, _, tag)| tag.name == subject)
                    .next()
                    .map(|(i, h, t)| (i, h.clone(), t.clone())),

                {
                    self.process_end_tag_in_body(Tag {
                        kind: EndTag,
                        name: subject,
                        self_closing: false,
                        attrs: vec!(),
                    });
                }
            );

            let fmt_elem_stack_index = unwrap_or_return!(
                self.open_elems.iter()
                    .rposition(|n| self.sink.same_node(n.clone(), fmt_elem.clone())),

                {
                    self.sink.parse_error(Borrowed("Formatting element not open"));
                    self.active_formatting.remove(fmt_elem_index);
                }
            );

            // 7.
            if !self.in_scope(default_scope, |n| self.sink.same_node(n.clone(), fmt_elem.clone())) {
                self.sink.parse_error(Borrowed("Formatting element not in scope"));
                return;
            }

            // 8.
            if !self.sink.same_node(self.current_node(), fmt_elem.clone()) {
                self.sink.parse_error(Borrowed("Formatting element not current node"));
            }

            // 9.
            let (furthest_block_index, furthest_block) = unwrap_or_return!(
                self.open_elems.iter()
                    .enumerate()
                    .skip(fmt_elem_stack_index)
                    .filter(|&(_, open_element)| self.elem_in(open_element.clone(), special_tag))
                    .next()
                    .map(|(i, h)| (i, h.clone())),

                // 10.
                {
                    self.open_elems.truncate(fmt_elem_stack_index);
                    self.active_formatting.remove(fmt_elem_index);
                }
            );

            // 11.
            let common_ancestor = self.open_elems[fmt_elem_stack_index - 1].clone();

            // 12.
            let mut bookmark = Bookmark::Replace(fmt_elem.clone());

            // 13.
            let mut node;
            let mut node_index = furthest_block_index;
            let mut last_node = furthest_block.clone();

            // 13.1.
            let mut inner_counter = 0;
            loop {
                // 13.2.
                inner_counter += 1;

                // 13.3.
                node_index -= 1;
                node = self.open_elems[node_index].clone();

                // 13.4.
                if self.sink.same_node(node.clone(), fmt_elem.clone()) {
                    break;
                }

                // 13.5.
                if inner_counter > 3 {
                    self.position_in_active_formatting(&node)
                        .map(|position| self.active_formatting.remove(position));
                    self.open_elems.remove(node_index);
                    continue;
                }

                let node_formatting_index = unwrap_or_else!(
                    self.position_in_active_formatting(&node),

                    // 13.6.
                    {
                        self.open_elems.remove(node_index);
                        continue;
                    }
                );

                // 13.7.
                let tag = match self.active_formatting[node_formatting_index] {
                    Element(ref h, ref t) => {
                        assert!(self.sink.same_node(h.clone(), node.clone()));
                        t.clone()
                    }
                    Marker => panic!("Found marker during adoption agency"),
                };
                // FIXME: Is there a way to avoid cloning the attributes twice here (once on their
                // own, once as part of t.clone() above)?
                let new_element = self.sink.create_element(
                    QualName::new(ns!(html), tag.name.clone()), tag.attrs.clone());
                self.open_elems[node_index] = new_element.clone();
                self.active_formatting[node_formatting_index] = Element(new_element.clone(), tag);
                node = new_element;

                // 13.8.
                if self.sink.same_node(last_node.clone(), furthest_block.clone()) {
                    bookmark = Bookmark::InsertAfter(node.clone());
                }

                // 13.9.
                self.sink.remove_from_parent(last_node.clone());
                self.sink.append(node.clone(), AppendNode(last_node.clone()));

                // 13.10.
                last_node = node.clone();

                // 13.11.
            }

            // 14.
            self.sink.remove_from_parent(last_node.clone());
            self.insert_appropriately(AppendNode(last_node.clone()), Some(common_ancestor));

            // 15.
            // FIXME: Is there a way to avoid cloning the attributes twice here (once on their own,
            // once as part of t.clone() above)?
            let new_element = self.sink.create_element(
                QualName::new(ns!(html), fmt_elem_tag.name.clone()), fmt_elem_tag.attrs.clone());
            let new_entry = Element(new_element.clone(), fmt_elem_tag);

            // 16.
            self.sink.reparent_children(furthest_block.clone(), new_element.clone());

            // 17.
            self.sink.append(furthest_block.clone(), AppendNode(new_element.clone()));

            // 18.
            // FIXME: We could probably get rid of the position_in_active_formatting() calls here
            // if we had a more clever Bookmark representation.
            match bookmark {
                Bookmark::Replace(to_replace) => {
                    let index = self.position_in_active_formatting(&to_replace)
                        .expect("bookmark not found in active formatting elements");
                    self.active_formatting[index] = new_entry;
                }
                Bookmark::InsertAfter(previous) => {
                    let index = self.position_in_active_formatting(&previous)
                        .expect("bookmark not found in active formatting elements") + 1;
                    self.active_formatting.insert(index, new_entry);
                    let old_index = self.position_in_active_formatting(&fmt_elem)
                        .expect("formatting element not found in active formatting elements");
                    self.active_formatting.remove(old_index);
                }
            }

            // 19.
            self.remove_from_stack(&fmt_elem);
            let new_furthest_block_index = self.open_elems.iter()
                .position(|n| self.sink.same_node(n.clone(), furthest_block.clone()))
                .expect("furthest block missing from open element stack");
            self.open_elems.insert(new_furthest_block_index + 1, new_element);

            // 20.
        }
    }

    fn push(&mut self, elem: &Handle) {
        self.open_elems.push(elem.clone());
    }

    fn pop(&mut self) -> Handle {
        let elem = self.open_elems.pop().expect("no current element");
        self.sink.pop(elem.clone());
        elem
    }

    fn remove_from_stack(&mut self, elem: &Handle) {
        let sink = &mut self.sink;
        let position = self.open_elems
            .iter()
            .rposition(|x| sink.same_node(elem.clone(), x.clone()));
        if let Some(position) = position {
            self.open_elems.remove(position);
            sink.pop(elem.clone());
        }
    }

    fn is_marker_or_open(&self, entry: &FormatEntry<Handle>) -> bool {
        match *entry {
            Marker => true,
            Element(ref node, _) => {
                self.open_elems.iter()
                    .rev()
                    .any(|n| self.sink.same_node(n.clone(), node.clone()))
            }
        }
    }

    /// Reconstruct the active formatting elements.
    fn reconstruct_formatting(&mut self) {
        {
            let last = unwrap_or_return!(self.active_formatting.last(), ());
            if self.is_marker_or_open(last) {
                return
            }
        }

        let mut entry_index = self.active_formatting.len() - 1;
        loop {
            if entry_index == 0 {
                break
            }
            entry_index -= 1;
            if self.is_marker_or_open(&self.active_formatting[entry_index]) {
                entry_index += 1;
                break
            }
        }

        loop {
            let tag = match self.active_formatting[entry_index] {
                Element(_, ref t) => t.clone(),
                Marker => panic!("Found marker during formatting element reconstruction"),
            };

            // FIXME: Is there a way to avoid cloning the attributes twice here (once on their own,
            // once as part of t.clone() above)?
            let new_element = self.insert_element(Push, ns!(html), tag.name.clone(),
                                                  tag.attrs.clone());
            self.active_formatting[entry_index] = Element(new_element, tag);
            if entry_index == self.active_formatting.len() - 1 {
                break
            }
            entry_index += 1;
        }
    }

    /// Get the first element on the stack, which will be the <html> element.
    fn html_elem(&self) -> Handle {
         self.open_elems[0].clone()
    }

    /// Get the second element on the stack, if it's a HTML body element.
    fn body_elem(&mut self) -> Option<Handle> {
        if self.open_elems.len() <= 1 {
            return None;
        }

        let node = self.open_elems[1].clone();
        if self.html_elem_named(node.clone(), local_name!("body")) {
            Some(node)
        } else {
            None
        }
    }

    /// Signal an error depending on the state of the stack of open elements at
    /// the end of the body.
    fn check_body_end(&mut self) {
        declare_tag_set!(body_end_ok =
            "dd" "dt" "li" "optgroup" "option" "p" "rp" "rt" "tbody" "td" "tfoot" "th"
            "thead" "tr" "body" "html");

        for elem in self.open_elems.iter() {
            let name = self.sink.elem_name(elem.clone());
            if !body_end_ok(name.clone()) {
                self.sink.parse_error(format_if!(self.opts.exact_errors,
                    "Unexpected open tag at end of body",
                    "Unexpected open tag {:?} at end of body", name));
                // FIXME: Do we keep checking after finding one bad tag?
                // The spec suggests not.
                return;
            }
        }
    }

    fn in_scope<TagSet,Pred>(&self, scope: TagSet, pred: Pred) -> bool
        where TagSet: Fn(QualName) -> bool, Pred: Fn(Handle) -> bool
    {
        for node in self.open_elems.iter().rev() {
            if pred(node.clone()) {
                return true;
            }
            if scope(self.sink.elem_name(node.clone())) {
                return false;
            }
        }

        // supposed to be impossible, because <html> is always in scope

        false
    }

    fn elem_in<TagSet>(&self, elem: Handle, set: TagSet) -> bool
        where TagSet: Fn(QualName) -> bool
    {
        set(self.sink.elem_name(elem))
    }

    fn html_elem_named(&self, elem: Handle, name: LocalName) -> bool {
        self.sink.elem_name(elem) == QualName::new(ns!(html), name)
    }

    fn in_html_elem_named(&self, name: LocalName) -> bool {
        self.open_elems.iter().any(|elem| self.html_elem_named(elem.clone(), name.clone()))
    }

    fn current_node_named(&self, name: LocalName) -> bool {
        self.html_elem_named(self.current_node(), name)
    }

    fn in_scope_named<TagSet>(&self, scope: TagSet, name: LocalName) -> bool
        where TagSet: Fn(QualName) -> bool
    {
        self.in_scope(scope, |elem|
            self.html_elem_named(elem, name.clone()))
    }

    //§ closing-elements-that-have-implied-end-tags
    fn generate_implied_end<TagSet>(&mut self, set: TagSet)
        where TagSet: Fn(QualName) -> bool
    {
        loop {
            let elem = unwrap_or_return!(self.open_elems.last(), ()).clone();
            let nsname = self.sink.elem_name(elem);
            if !set(nsname) { return; }
            self.pop();
        }
    }

    fn generate_implied_end_except(&mut self, except: LocalName) {
        self.generate_implied_end(|p| match p {
            QualName { ns: ns!(html), ref local } if *local == except => false,
            _ => cursory_implied_end(p),
        });
    }
    //§ END

    // Pop elements until the current element is in the set.
    fn pop_until_current<TagSet>(&mut self, pred: TagSet)
        where TagSet: Fn(QualName) -> bool
    {
        loop {
            if self.current_node_in(|x| pred(x)) {
                break;
            }
            self.open_elems.pop();
        }
    }

    // Pop elements until an element from the set has been popped.  Returns the
    // number of elements popped.
    fn pop_until<P>(&mut self, pred: P) -> usize
        where P: Fn(QualName) -> bool
    {
        let mut n = 0;
        loop {
            n += 1;
            match self.open_elems.pop() {
                None => break,
                Some(elem) => if pred(self.sink.elem_name(elem)) { break; },
            }
        }
        n
    }

    fn pop_until_named(&mut self, name: LocalName) -> usize {
        self.pop_until(|p| p == QualName::new(ns!(html), name.clone()))
    }

    // Pop elements until one with the specified name has been popped.
    // Signal an error if it was not the first one.
    fn expect_to_close(&mut self, name: LocalName) {
        if self.pop_until_named(name.clone()) != 1 {
            self.sink.parse_error(format_if!(self.opts.exact_errors,
                "Unexpected open element",
                "Unexpected open element while closing {:?}", name));
        }
    }

    fn close_p_element(&mut self) {
        declare_tag_set!(implied = [cursory_implied_end] - "p");
        self.generate_implied_end(implied);
        self.expect_to_close(local_name!("p"));
    }

    fn close_p_element_in_button_scope(&mut self) {
        if self.in_scope_named(button_scope, local_name!("p")) {
            self.close_p_element();
        }
    }

    // Check <input> tags for type=hidden
    fn is_type_hidden(&self, tag: &Tag) -> bool {
        match tag.attrs.iter().find(|&at| at.name == qualname!("", "type")) {
            None => false,
            Some(at) => (&*at.value).eq_ignore_ascii_case("hidden"),
        }
    }

    fn foster_parent_in_body(&mut self, token: Token) -> ProcessResult<Handle> {
        warn!("foster parenting not implemented");
        self.foster_parenting = true;
        let res = self.step(InBody, token);
        // FIXME: what if res is Reprocess?
        self.foster_parenting = false;
        res
    }

    fn process_chars_in_table(&mut self, token: Token) -> ProcessResult<Handle> {
        declare_tag_set!(table_outer = "table" "tbody" "tfoot" "thead" "tr");
        if self.current_node_in(table_outer) {
            assert!(self.pending_table_text.is_empty());
            self.orig_mode = Some(self.mode);
            Reprocess(InTableText, token)
        } else {
            self.sink.parse_error(format_if!(self.opts.exact_errors,
                "Unexpected characters in table",
                "Unexpected characters {} in table", to_escaped_string(&token)));
            self.foster_parent_in_body(token)
        }
    }

    // https://html.spec.whatwg.org/multipage/syntax.html#reset-the-insertion-mode-appropriately
    fn reset_insertion_mode(&mut self) -> InsertionMode {
        for (i, mut node) in self.open_elems.iter().enumerate().rev() {
            let last = i == 0usize;
            if let (true, Some(ctx)) = (last, self.context_elem.as_ref()) {
                node = ctx;
            }
            let name = match self.sink.elem_name(node.clone()) {
                QualName { ns: ns!(html), local } => local,
                _ => continue,
            };
            match name {
                local_name!("select") => {
                    for ancestor in self.open_elems[0..i].iter().rev() {
                        if self.html_elem_named(ancestor.clone(), local_name!("template")) {
                            return InSelect;
                        } else if self.html_elem_named(ancestor.clone(), local_name!("table")) {
                            return InSelectInTable;
                        }
                    }
                    return InSelect;
                },
                local_name!("td") | local_name!("th") => if !last { return InCell; },
                local_name!("tr") => return InRow,
                local_name!("tbody") | local_name!("thead") | local_name!("tfoot") => return InTableBody,
                local_name!("caption") => return InCaption,
                local_name!("colgroup") => return InColumnGroup,
                local_name!("table") => return InTable,
                local_name!("template") => return *self.template_modes.last().unwrap(),
                local_name!("head") => if !last { return InHead },
                local_name!("body") => return InBody,
                local_name!("frameset") => return InFrameset,
                local_name!("html") => match self.head_elem {
                    None => return BeforeHead,
                    Some(_) => return AfterHead,
                },

                _ => (),
            }
        }
        InBody
    }

    fn close_the_cell(&mut self) {
        self.generate_implied_end(cursory_implied_end);
        if self.pop_until(td_th) != 1 {
            self.sink.parse_error(Borrowed("expected to close <td> or <th> with cell"));
        }
        self.clear_active_formatting_to_marker();
    }

    fn append_text(&mut self, text: StrTendril) -> ProcessResult<Handle> {
        self.insert_appropriately(AppendText(text), None);
        Done
    }

    fn append_comment(&mut self, text: StrTendril) -> ProcessResult<Handle> {
        let comment = self.sink.create_comment(text);
        self.insert_appropriately(AppendNode(comment), None);
        Done
    }

    fn append_comment_to_doc(&mut self, text: StrTendril) -> ProcessResult<Handle> {
        let target = self.doc_handle.clone();
        let comment = self.sink.create_comment(text);
        self.sink.append(target, AppendNode(comment));
        Done
    }

    fn append_comment_to_html(&mut self, text: StrTendril) -> ProcessResult<Handle> {
        let target = self.html_elem();
        let comment = self.sink.create_comment(text);
        self.sink.append(target, AppendNode(comment));
        Done
    }

    //§ creating-and-inserting-nodes
    fn create_root(&mut self, attrs: Vec<Attribute>) {
        let elem = self.sink.create_element(qualname!(html, "html"), attrs);
        self.push(&elem);
        self.sink.append(self.doc_handle.clone(), AppendNode(elem));
        // FIXME: application cache selection algorithm
    }

    // https://html.spec.whatwg.org/multipage/#create-an-element-for-the-token
    fn insert_element(&mut self, push: PushFlag, ns: Namespace, name: LocalName, attrs: Vec<Attribute>)
            -> Handle {
        declare_tag_set!(form_associatable =
            "button" "fieldset" "input" "object"
            "output" "select" "textarea" "img");

        declare_tag_set!(listed = [form_associatable] - "img");

        // Step 7.
        let qname = QualName::new(ns, name);
        let elem = self.sink.create_element(qname.clone(), attrs.clone());

        let insertion_point = self.appropriate_place_for_insertion(None);
        let tree_node = match insertion_point {
            LastChild(ref p) |
            BeforeSibling(ref p) => p.clone()
        };

        // Step 12.
        if form_associatable(qname.clone()) &&
           self.form_elem.is_some() &&
           !self.in_html_elem_named(local_name!("template")) &&
           !(listed(qname.clone()) &&
                attrs.iter().any(|a| a.name == qualname!("","form"))) {

               let form = self.form_elem.as_ref().unwrap().clone();
               if self.sink.same_tree(tree_node, form.clone()) {
                   self.sink.associate_with_form(elem.clone(), form)
               }
        }

        self.insert_at(insertion_point, AppendNode(elem.clone()));

        match push {
            Push => self.push(&elem),
            NoPush => (),
        }
        // FIXME: Remove from the stack if we can't append?
        elem
    }

    fn insert_element_for(&mut self, tag: Tag) -> Handle {
        self.insert_element(Push, ns!(html), tag.name, tag.attrs)
    }

    fn insert_and_pop_element_for(&mut self, tag: Tag) -> Handle {
        self.insert_element(NoPush, ns!(html), tag.name, tag.attrs)
    }

    fn insert_phantom(&mut self, name: LocalName) -> Handle {
        self.insert_element(Push, ns!(html), name, vec!())
    }
    //§ END

    fn create_formatting_element_for(&mut self, tag: Tag) -> Handle {
        // FIXME: This really wants unit tests.
        let mut first_match = None;
        let mut matches = 0usize;
        for (i, _, old_tag) in self.active_formatting_end_to_marker() {
            if tag.equiv_modulo_attr_order(old_tag) {
                first_match = Some(i);
                matches += 1;
            }
        }

        if matches >= 3 {
            self.active_formatting.remove(first_match.expect("matches with no index"));
        }

        let elem = self.insert_element(Push, ns!(html), tag.name.clone(), tag.attrs.clone());
        self.active_formatting.push(Element(elem.clone(), tag));
        elem
    }

    fn clear_active_formatting_to_marker(&mut self) {
        loop {
            match self.active_formatting.pop() {
                None | Some(Marker) => break,
                _ => (),
            }
        }
    }

    fn process_end_tag_in_body(&mut self, tag: Tag) {
        // Look back for a matching open element.
        let mut match_idx = None;
        for (i, elem) in self.open_elems.iter().enumerate().rev() {
            if self.html_elem_named(elem.clone(), tag.name.clone()) {
                match_idx = Some(i);
                break;
            }

            if self.elem_in(elem.clone(), special_tag) {
                self.sink.parse_error(Borrowed("Found special tag while closing generic tag"));
                return;
            }
        }

        // Can't use unwrap_or_return!() due to rust-lang/rust#16617.
        let match_idx = match match_idx {
            None => {
                // I believe this is impossible, because the root
                // <html> element is in special_tag.
                self.unexpected(&tag);
                return;
            }
            Some(x) => x,
        };

        self.generate_implied_end_except(tag.name.clone());

        if match_idx != self.open_elems.len() - 1 {
            // mis-nested tags
            self.unexpected(&tag);
        }
        self.open_elems.truncate(match_idx);
    }

    fn handle_misnested_a_tags(&mut self, tag: &Tag) {
        let node = unwrap_or_return!(
            self.active_formatting_end_to_marker()
                .filter(|&(_, n, _)| self.html_elem_named(n.clone(), local_name!("a")))
                .next()
                .map(|(_, n, _)| n.clone()),

            ()
        );

        self.unexpected(tag);
        self.adoption_agency(local_name!("a"));
        self.position_in_active_formatting(&node)
            .map(|index| self.active_formatting.remove(index));
        self.remove_from_stack(&node);
    }

    //§ tree-construction
    fn is_foreign(&mut self, token: &Token) -> bool {
        if let EOFToken = *token {
            return false;
        }

        if self.open_elems.len() == 0 {
            return false;
        }

        let name = self.sink.elem_name(self.adjusted_current_node());
        if let ns!(html) = name.ns {
            return false;
        }

        if mathml_text_integration_point(name.clone()) {
            match *token {
                CharacterTokens(..) | NullCharacterToken => return false,
                TagToken(Tag { kind: StartTag, ref name, .. })
                    if !matches!(*name, local_name!("mglyph") | local_name!("malignmark")) => return false,
                _ => (),
            }
        }

        if svg_html_integration_point(name.clone()) {
            match *token {
                CharacterTokens(..) | NullCharacterToken => return false,
                TagToken(Tag { kind: StartTag, .. }) => return false,
                _ => (),
            }
        }

        if let qualname!(mathml, "annotation-xml") = name {
            match *token {
                TagToken(Tag { kind: StartTag, name: local_name!("svg"), .. }) => return false,
                CharacterTokens(..) | NullCharacterToken |
                TagToken(Tag { kind: StartTag, .. }) => {
                    return !self.sink.is_mathml_annotation_xml_integration_point(self.adjusted_current_node())
                }
                _ => {}
            };
        }

        true
    }
    //§ END

    fn enter_foreign(&mut self, mut tag: Tag, ns: Namespace) -> ProcessResult<Handle> {
        match ns {
            ns!(mathml) => self.adjust_mathml_attributes(&mut tag),
            ns!(svg) => self.adjust_svg_attributes(&mut tag),
            _ => (),
        }
        self.adjust_foreign_attributes(&mut tag);

        if tag.self_closing {
            self.insert_element(NoPush, ns, tag.name, tag.attrs);
            DoneAckSelfClosing
        } else {
            self.insert_element(Push, ns, tag.name, tag.attrs);
            Done
        }
    }

    fn adjust_svg_tag_name(&mut self, tag: &mut Tag) {
        let Tag { ref mut name, .. } = *tag;
        match *name {
            local_name!("altglyph") => *name = local_name!("altGlyph"),
            local_name!("altglyphdef") => *name = local_name!("altGlyphDef"),
            local_name!("altglyphitem") => *name = local_name!("altGlyphItem"),
            local_name!("animatecolor") => *name = local_name!("animateColor"),
            local_name!("animatemotion") => *name = local_name!("animateMotion"),
            local_name!("animatetransform") => *name = local_name!("animateTransform"),
            local_name!("clippath") => *name = local_name!("clipPath"),
            local_name!("feblend") => *name = local_name!("feBlend"),
            local_name!("fecolormatrix") => *name = local_name!("feColorMatrix"),
            local_name!("fecomponenttransfer") => *name = local_name!("feComponentTransfer"),
            local_name!("fecomposite") => *name = local_name!("feComposite"),
            local_name!("feconvolvematrix") => *name = local_name!("feConvolveMatrix"),
            local_name!("fediffuselighting") => *name = local_name!("feDiffuseLighting"),
            local_name!("fedisplacementmap") => *name = local_name!("feDisplacementMap"),
            local_name!("fedistantlight") => *name = local_name!("feDistantLight"),
            local_name!("fedropshadow") => *name = local_name!("feDropShadow"),
            local_name!("feflood") => *name = local_name!("feFlood"),
            local_name!("fefunca") => *name = local_name!("feFuncA"),
            local_name!("fefuncb") => *name = local_name!("feFuncB"),
            local_name!("fefuncg") => *name = local_name!("feFuncG"),
            local_name!("fefuncr") => *name = local_name!("feFuncR"),
            local_name!("fegaussianblur") => *name = local_name!("feGaussianBlur"),
            local_name!("feimage") => *name = local_name!("feImage"),
            local_name!("femerge") => *name = local_name!("feMerge"),
            local_name!("femergenode") => *name = local_name!("feMergeNode"),
            local_name!("femorphology") => *name = local_name!("feMorphology"),
            local_name!("feoffset") => *name = local_name!("feOffset"),
            local_name!("fepointlight") => *name = local_name!("fePointLight"),
            local_name!("fespecularlighting") => *name = local_name!("feSpecularLighting"),
            local_name!("fespotlight") => *name = local_name!("feSpotLight"),
            local_name!("fetile") => *name = local_name!("feTile"),
            local_name!("feturbulence") => *name = local_name!("feTurbulence"),
            local_name!("foreignobject") => *name = local_name!("foreignObject"),
            local_name!("glyphref") => *name = local_name!("glyphRef"),
            local_name!("lineargradient") => *name = local_name!("linearGradient"),
            local_name!("radialgradient") => *name = local_name!("radialGradient"),
            local_name!("textpath") => *name = local_name!("textPath"),
            _ => (),
        }
    }

    fn adjust_attributes<F>(&mut self, tag: &mut Tag, mut map: F)
        where F: FnMut(LocalName) -> Option<QualName>,
    {
        for &mut Attribute { ref mut name, .. } in &mut tag.attrs {
            if let Some(replacement) = map(name.local.clone()) {
                *name = replacement;
            }
        }
    }

    fn adjust_svg_attributes(&mut self, tag: &mut Tag) {
        self.adjust_attributes(tag, |k| match k {
            local_name!("attributename") => Some(qualname!("", "attributeName")),
            local_name!("attributetype") => Some(qualname!("", "attributeType")),
            local_name!("basefrequency") => Some(qualname!("", "baseFrequency")),
            local_name!("baseprofile") => Some(qualname!("", "baseProfile")),
            local_name!("calcmode") => Some(qualname!("", "calcMode")),
            local_name!("clippathunits") => Some(qualname!("", "clipPathUnits")),
            local_name!("diffuseconstant") => Some(qualname!("", "diffuseConstant")),
            local_name!("edgemode") => Some(qualname!("", "edgeMode")),
            local_name!("filterunits") => Some(qualname!("", "filterUnits")),
            local_name!("glyphref") => Some(qualname!("", "glyphRef")),
            local_name!("gradienttransform") => Some(qualname!("", "gradientTransform")),
            local_name!("gradientunits") => Some(qualname!("", "gradientUnits")),
            local_name!("kernelmatrix") => Some(qualname!("", "kernelMatrix")),
            local_name!("kernelunitlength") => Some(qualname!("", "kernelUnitLength")),
            local_name!("keypoints") => Some(qualname!("", "keyPoints")),
            local_name!("keysplines") => Some(qualname!("", "keySplines")),
            local_name!("keytimes") => Some(qualname!("", "keyTimes")),
            local_name!("lengthadjust") => Some(qualname!("", "lengthAdjust")),
            local_name!("limitingconeangle") => Some(qualname!("", "limitingConeAngle")),
            local_name!("markerheight") => Some(qualname!("", "markerHeight")),
            local_name!("markerunits") => Some(qualname!("", "markerUnits")),
            local_name!("markerwidth") => Some(qualname!("", "markerWidth")),
            local_name!("maskcontentunits") => Some(qualname!("", "maskContentUnits")),
            local_name!("maskunits") => Some(qualname!("", "maskUnits")),
            local_name!("numoctaves") => Some(qualname!("", "numOctaves")),
            local_name!("pathlength") => Some(qualname!("", "pathLength")),
            local_name!("patterncontentunits") => Some(qualname!("", "patternContentUnits")),
            local_name!("patterntransform") => Some(qualname!("", "patternTransform")),
            local_name!("patternunits") => Some(qualname!("", "patternUnits")),
            local_name!("pointsatx") => Some(qualname!("", "pointsAtX")),
            local_name!("pointsaty") => Some(qualname!("", "pointsAtY")),
            local_name!("pointsatz") => Some(qualname!("", "pointsAtZ")),
            local_name!("preservealpha") => Some(qualname!("", "preserveAlpha")),
            local_name!("preserveaspectratio") => Some(qualname!("", "preserveAspectRatio")),
            local_name!("primitiveunits") => Some(qualname!("", "primitiveUnits")),
            local_name!("refx") => Some(qualname!("", "refX")),
            local_name!("refy") => Some(qualname!("", "refY")),
            local_name!("repeatcount") => Some(qualname!("", "repeatCount")),
            local_name!("repeatdur") => Some(qualname!("", "repeatDur")),
            local_name!("requiredextensions") => Some(qualname!("", "requiredExtensions")),
            local_name!("requiredfeatures") => Some(qualname!("", "requiredFeatures")),
            local_name!("specularconstant") => Some(qualname!("", "specularConstant")),
            local_name!("specularexponent") => Some(qualname!("", "specularExponent")),
            local_name!("spreadmethod") => Some(qualname!("", "spreadMethod")),
            local_name!("startoffset") => Some(qualname!("", "startOffset")),
            local_name!("stddeviation") => Some(qualname!("", "stdDeviation")),
            local_name!("stitchtiles") => Some(qualname!("", "stitchTiles")),
            local_name!("surfacescale") => Some(qualname!("", "surfaceScale")),
            local_name!("systemlanguage") => Some(qualname!("", "systemLanguage")),
            local_name!("tablevalues") => Some(qualname!("", "tableValues")),
            local_name!("targetx") => Some(qualname!("", "targetX")),
            local_name!("targety") => Some(qualname!("", "targetY")),
            local_name!("textlength") => Some(qualname!("", "textLength")),
            local_name!("viewbox") => Some(qualname!("", "viewBox")),
            local_name!("viewtarget") => Some(qualname!("", "viewTarget")),
            local_name!("xchannelselector") => Some(qualname!("", "xChannelSelector")),
            local_name!("ychannelselector") => Some(qualname!("", "yChannelSelector")),
            local_name!("zoomandpan") => Some(qualname!("", "zoomAndPan")),
            _ => None,
        });
    }

    fn adjust_mathml_attributes(&mut self, tag: &mut Tag) {
        self.adjust_attributes(tag, |k| match k {
            local_name!("definitionurl") => Some(qualname!("", "definitionURL")),
            _ => None,
        });
    }

    fn adjust_foreign_attributes(&mut self, tag: &mut Tag) {
        self.adjust_attributes(tag, |k| match k {
            local_name!("xlink:actuate") => Some(qualname!(xlink, "actuate")),
            local_name!("xlink:arcrole") => Some(qualname!(xlink, "arcrole")),
            local_name!("xlink:href") => Some(qualname!(xlink, "href")),
            local_name!("xlink:role") => Some(qualname!(xlink, "role")),
            local_name!("xlink:show") => Some(qualname!(xlink, "show")),
            local_name!("xlink:title") => Some(qualname!(xlink, "title")),
            local_name!("xlink:type") => Some(qualname!(xlink, "type")),
            local_name!("xml:base") => Some(qualname!(xml, "base")),
            local_name!("xml:lang") => Some(qualname!(xml, "lang")),
            local_name!("xml:space") => Some(qualname!(xml, "space")),
            local_name!("xmlns") => Some(qualname!(xmlns, "xmlns")),
            local_name!("xmlns:xlink") => Some(qualname!(xmlns, "xlink")),
            _ => None,
        });
    }

    fn foreign_start_tag(&mut self, mut tag: Tag) -> ProcessResult<Handle> {
        let cur = self.sink.elem_name(self.adjusted_current_node());
        match cur.ns {
            ns!(mathml) => self.adjust_mathml_attributes(&mut tag),
            ns!(svg) => {
                self.adjust_svg_tag_name(&mut tag);
                self.adjust_svg_attributes(&mut tag);
            }
            _ => (),
        }
        self.adjust_foreign_attributes(&mut tag);
        if tag.self_closing {
            // FIXME(#118): <script /> in SVG
            self.insert_element(NoPush, cur.ns, tag.name, tag.attrs);
            DoneAckSelfClosing
        } else {
            self.insert_element(Push, cur.ns, tag.name, tag.attrs);
            Done
        }
    }

    fn unexpected_start_tag_in_foreign_content(&mut self, tag: Tag) -> ProcessResult<Handle> {
        self.unexpected(&tag);
        if self.is_fragment() {
            self.foreign_start_tag(tag)
        } else {
            self.pop();
            while !self.current_node_in(|n| {
                n.ns == ns!(html) || mathml_text_integration_point(n.clone())
                    || svg_html_integration_point(n)
            }) {
                self.pop();
            }
            ReprocessForeign(TagToken(tag))
        }
    }
}
