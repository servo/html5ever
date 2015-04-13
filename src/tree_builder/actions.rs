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

use tokenizer::{XTag, XPi};

use util::str::{AsciiExt, to_escaped_string};

use std::{slice, fmt};
use std::mem::replace;
use std::iter::{Rev, Enumerate};
use std::borrow::Cow::Borrowed;

use string_cache::{Atom, Namespace, QualName};

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
    fn unexpected<T: fmt::Debug>(&mut self, thing: &T) -> ProcessResult;
    fn assert_named(&mut self, node: Handle, name: Atom);
    fn clear_active_formatting_to_marker(&mut self);
    fn create_formatting_element_for(&mut self, tag: Tag) -> Handle;
    fn append_text(&mut self, text: String) -> ProcessResult;
    fn append_comment(&mut self, text: String) -> ProcessResult;
    fn append_comment_to_doc(&mut self, text: String) -> ProcessResult;
    fn append_comment_to_html(&mut self, text: String) -> ProcessResult;
    fn insert_appropriately(&mut self, child: NodeOrText<Handle>, override_target: Option<Handle>);
    fn insert_phantom(&mut self, name: Atom) -> Handle;
    fn insert_and_pop_element_for(&mut self, tag: Tag) -> Handle;
    fn insert_element_for(&mut self, tag: Tag) -> Handle;
    fn insert_element(&mut self, push: PushFlag, ns: Namespace, name: Atom, attrs: Vec<Attribute>) -> Handle;
    fn create_root(&mut self, attrs: Vec<Attribute>);
    fn close_the_cell(&mut self);
    fn reset_insertion_mode(&mut self) -> InsertionMode;
    fn process_chars_in_table(&mut self, token: Token) -> ProcessResult;
    fn foster_parent_in_body(&mut self, token: Token) -> ProcessResult;
    fn is_type_hidden(&self, tag: &Tag) -> bool;
    fn close_p_element_in_button_scope(&mut self);
    fn close_p_element(&mut self);
    fn expect_to_close(&mut self, name: Atom);
    fn pop_until_named(&mut self, name: Atom) -> usize;
    fn pop_until<TagSet>(&mut self, pred: TagSet) -> usize where TagSet: Fn(QualName) -> bool;
    fn pop_until_current<TagSet>(&mut self, pred: TagSet) where TagSet: Fn(QualName) -> bool;
    fn generate_implied_end_except(&mut self, except: Atom);
    fn generate_implied_end<TagSet>(&mut self, set: TagSet) where TagSet: Fn(QualName) -> bool;
    fn in_scope_named<TagSet>(&self, scope: TagSet, name: Atom) -> bool where TagSet: Fn(QualName) -> bool;
    fn current_node_named(&self, name: Atom) -> bool;
    fn html_elem_named(&self, elem: Handle, name: Atom) -> bool;
    fn elem_in<TagSet>(&self, elem: Handle, set: TagSet) -> bool where TagSet: Fn(QualName) -> bool;
    fn in_scope<TagSet,Pred>(&self, scope: TagSet, pred: Pred) -> bool where TagSet: Fn(QualName) -> bool, Pred: Fn(Handle) -> bool;
    fn check_body_end(&mut self);
    fn body_elem(&mut self) -> Option<Handle>;
    fn html_elem(&self) -> Handle;
    fn reconstruct_formatting(&mut self);
    fn remove_from_stack(&mut self, elem: &Handle);
    fn pop(&mut self) -> Handle;
    fn push(&mut self, elem: &Handle);
    fn adoption_agency(&mut self, subject: Atom);
    fn current_node_in<TagSet>(&self, set: TagSet) -> bool where TagSet: Fn(QualName) -> bool;
    fn current_node(&self) -> Handle;
    fn adjusted_current_node(&self) -> Handle;
    fn parse_raw_data(&mut self, tag: Tag, k: RawKind);
    fn to_raw_text_mode(&mut self, k: RawKind);
    fn stop_parsing(&mut self) -> ProcessResult;
    fn set_quirks_mode(&mut self, mode: QuirksMode);
    fn active_formatting_end_to_marker<'a>(&'a self) -> ActiveFormattingIter<'a, Handle>;
    fn is_marker_or_open(&self, entry: &FormatEntry<Handle>) -> bool;
    fn position_in_active_formatting(&self, element: &Handle) -> Option<usize>;
    fn process_end_tag_in_body(&mut self, tag: Tag);
    fn handle_misnested_a_tags(&mut self, tag: &Tag);
    fn is_foreign(&mut self, token: &Token) -> bool;
    fn enter_foreign(&mut self, tag: Tag, ns: Namespace) -> ProcessResult;
    fn adjust_attributes<F>(&mut self, tag: &mut Tag, mut map: F)
        where F: FnMut(Atom) -> Option<QualName>;
    fn adjust_svg_tag_name(&mut self, tag: &mut Tag);
    fn adjust_svg_attributes(&mut self, tag: &mut Tag);
    fn adjust_mathml_attributes(&mut self, tag: &mut Tag);
    fn adjust_foreign_attributes(&mut self, tag: &mut Tag);
    fn foreign_start_tag(&mut self, tag: Tag) -> ProcessResult;
}

#[doc(hidden)]
impl<Handle, Sink> TreeBuilderActions<Handle>
    for super::TreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{
    fn unexpected<T: fmt::Debug>(&mut self, _thing: &T) -> ProcessResult {
        self.sink.parse_error(format_if!(
            self.opts.exact_errors,
            "Unexpected token",
            "Unexpected token {} in insertion mode {:?}", to_escaped_string(_thing), self.mode));
        Done
    }

    fn assert_named(&mut self, node: Handle, name: Atom) {
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

    fn stop_parsing(&mut self) -> ProcessResult {
        warn!("stop_parsing not implemented, full speed ahead!");
        Done
    }

    //§ parsing-elements-that-contain-only-text
    // Switch to `Text` insertion mode, save the old mode, and
    // switch the tokenizer to a raw-data state.
    // The latter only takes effect after the current / next
    // `process_token` of a start tag returns!
    fn to_raw_text_mode(&mut self, k: RawKind) {
        assert!(self.next_tokenizer_state.is_none());
        self.next_tokenizer_state = Some(RawData(k));
        self.orig_mode = Some(self.mode);
        self.mode = Text;
    }

    // The generic raw text / RCDATA parsing algorithm.
    fn parse_raw_data(&mut self, tag: Tag, k: RawKind) {
        self.insert_element_for(tag);
        self.to_raw_text_mode(k);
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
        set(self.sink.elem_name(&self.current_node()))
    }

    // Insert at the "appropriate place for inserting a node".
    fn insert_appropriately(&mut self, child: NodeOrText<Handle>, override_target: Option<Handle>) {
        declare_tag_set!(foster_target = table tbody tfoot thead tr);
        let target = override_target.unwrap_or_else(|| self.current_node());
        if !(self.foster_parenting && self.elem_in(target.clone(), foster_target)) {
            // No foster parenting (the common case).
            return self.sink.append(target, child);
        }

        // Foster parenting
        // FIXME: <template>
        let last_table = self.open_elems.iter()
            .enumerate()
            .rev()
            .filter(|&(_, e)| self.html_elem_named(e.clone(), atom!(table)))
            .next();

        match last_table {
            None => {
                let html_elem = self.html_elem();
                self.sink.append(html_elem, child);
            }
            Some((idx, last_table)) => {
                // Try inserting "inside last table's parent node, immediately before last table"
                match self.sink.append_before_sibling(last_table.clone(), child) {
                    Ok(()) => (),

                    // If last_table has no parent, we regain ownership of the child.
                    // Insert "inside previous element, after its last child (if any)"
                    Err(child) => {
                        let previous_element = self.open_elems[idx-1].clone();
                        self.sink.append(previous_element, child);
                    }
                }
            }
        }
    }

    fn adoption_agency(&mut self, subject: Atom) {
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
                    QualName::new(ns!(HTML), tag.name.clone()), tag.attrs.clone());
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
                QualName::new(ns!(HTML), fmt_elem_tag.name.clone()), fmt_elem_tag.attrs.clone());
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
        self.open_elems.pop().expect("no current element")
    }

    fn remove_from_stack(&mut self, elem: &Handle) {
        let mut open_elems = replace(&mut self.open_elems, vec!());
        open_elems.retain(|x| !self.sink.same_node(elem.clone(), x.clone()));
        self.open_elems = open_elems;
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
            let new_element = self.insert_element(Push, ns!(HTML), tag.name.clone(),
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
        if self.html_elem_named(node.clone(), atom!(body)) {
            Some(node)
        } else {
            None
        }
    }

    /// Signal an error depending on the state of the stack of open elements at
    /// the end of the body.
    fn check_body_end(&mut self) {
        declare_tag_set!(body_end_ok =
            dd dt li optgroup option p rp rt tbody td tfoot th
            thead tr body html);

        for elem in self.open_elems.iter() {
            let name = self.sink.elem_name(&elem);
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
            if scope(self.sink.elem_name(&node)) {
                return false;
            }
        }

        // supposed to be impossible, because <html> is always in scope

        false
    }

    fn elem_in<TagSet>(&self, elem: Handle, set: TagSet) -> bool 
        where TagSet: Fn(QualName) -> bool
    {
        set(self.sink.elem_name(&elem))
    }

    fn html_elem_named(&self, elem: Handle, name: Atom) -> bool {
        self.sink.elem_name(&elem) == QualName::new(ns!(HTML), name)
    }

    fn current_node_named(&self, name: Atom) -> bool {
        self.html_elem_named(self.current_node(), name)
    }

    fn in_scope_named<TagSet>(&self, scope: TagSet, name: Atom) -> bool 
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
            let nsname = self.sink.elem_name(&elem);
            if !set(nsname) { return; }
            self.pop();
        }
    }

    fn generate_implied_end_except(&mut self, except: Atom) {
        self.generate_implied_end(|p| match p {
            QualName { ns: ns!(HTML), ref local } if *local == except => false,
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
                Some(ref elem) => if pred(self.sink.elem_name(elem)) { break; },
            }
        }
        n
    }

    fn pop_until_named(&mut self, name: Atom) -> usize {
        self.pop_until(|p| p == QualName::new(ns!(HTML), name.clone()))
    }

    // Pop elements until one with the specified name has been popped.
    // Signal an error if it was not the first one.
    fn expect_to_close(&mut self, name: Atom) {
        if self.pop_until_named(name.clone()) != 1 {
            self.sink.parse_error(format_if!(self.opts.exact_errors,
                "Unexpected open element",
                "Unexpected open element while closing {:?}", name));
        }
    }

    fn close_p_element(&mut self) {
        declare_tag_set!(implied = cursory_implied_end - p);
        self.generate_implied_end(implied);
        self.expect_to_close(atom!(p));
    }

    fn close_p_element_in_button_scope(&mut self) {
        if self.in_scope_named(button_scope, atom!(p)) {
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

    fn foster_parent_in_body(&mut self, token: Token) -> ProcessResult {
        warn!("foster parenting not implemented");
        self.foster_parenting = true;
        let res = self.step(InBody, token);
        // FIXME: what if res is Reprocess?
        self.foster_parenting = false;
        res
    }

    fn process_chars_in_table(&mut self, token: Token) -> ProcessResult {
        declare_tag_set!(table_outer = table tbody tfoot thead tr);
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
            let name = match self.sink.elem_name(&node) {
                QualName { ns: ns!(HTML), local } => local,
                _ => continue,
            };
            match name {
                // FIXME: <select> sub-steps
                atom!(select) => return InSelect,

                atom!(td) | atom!(th) => if !last { return InCell; },
                atom!(tr) => return InRow,
                atom!(tbody) | atom!(thead) | atom!(tfoot) => return InTableBody,
                atom!(caption) => return InCaption,
                atom!(colgroup) => return InColumnGroup,
                atom!(table) => return InTable,
                atom!(head) => if !last { return InHead },
                atom!(body) => return InBody,
                atom!(frameset) => return InFrameset,
                atom!(html) => match self.head_elem {
                    None => return BeforeHead,
                    Some(_) => return AfterHead,
                },

                atom!(template) => panic!("FIXME: <template> not implemented"),

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

    fn append_text(&mut self, text: String) -> ProcessResult {
        self.insert_appropriately(AppendText(text), None);
        Done
    }

    fn append_comment(&mut self, text: String) -> ProcessResult {
        let comment = self.sink.create_comment(text);
        self.insert_appropriately(AppendNode(comment), None);
        Done
    }

    fn append_comment_to_doc(&mut self, text: String) -> ProcessResult {
        let target = self.doc_handle.clone();
        let comment = self.sink.create_comment(text);
        self.sink.append(target, AppendNode(comment));
        Done
    }

    fn append_comment_to_html(&mut self, text: String) -> ProcessResult {
        let target = self.html_elem();
        let comment = self.sink.create_comment(text);
        self.sink.append(target, AppendNode(comment));
        Done
    }

    //§ creating-and-inserting-nodes
    fn create_root(&mut self, attrs: Vec<Attribute>) {
        let elem = self.sink.create_element(qualname!(HTML, html), attrs);
        self.push(&elem);
        self.sink.append(self.doc_handle.clone(), AppendNode(elem));
        // FIXME: application cache selection algorithm
    }

    fn insert_element(&mut self, push: PushFlag, ns: Namespace, name: Atom, attrs: Vec<Attribute>)
            -> Handle {
        let elem = self.sink.create_element(QualName::new(ns, name), attrs);
        self.insert_appropriately(AppendNode(elem.clone()), None);
        match push {
            Push => self.push(&elem),
            NoPush => (),
        }
        // FIXME: Remove from the stack if we can't append?
        elem
    }

    fn insert_element_for(&mut self, tag: Tag) -> Handle {
        self.insert_element(Push, ns!(HTML), tag.name, tag.attrs)
    }

    fn insert_and_pop_element_for(&mut self, tag: Tag) -> Handle {
        self.insert_element(NoPush, ns!(HTML), tag.name, tag.attrs)
    }

    fn insert_phantom(&mut self, name: Atom) -> Handle {
        self.insert_element(Push, ns!(HTML), name, vec!())
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

        let elem = self.insert_element(Push, ns!(HTML), tag.name.clone(), tag.attrs.clone());
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
                .filter(|&(_, n, _)| self.html_elem_named(n.clone(), atom!(a)))
                .next()
                .map(|(_, n, _)| n.clone()),

            ()
        );

        self.unexpected(tag);
        self.adoption_agency(atom!(a));
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

        let name = self.sink.elem_name(&self.adjusted_current_node());
        if let ns!(HTML) = name.ns {
            return false;
        }

        if mathml_text_integration_point(name.clone()) {
            match *token {
                CharacterTokens(..) => return false,
                TagToken(Tag { kind: StartTag, ref name, .. })
                    if !matches!(*name, atom!(mglyph) | atom!(malignmark)) => return false,
                _ => (),
            }
        }

        if let qualname!(MathML, "annotation-xml") = name {
            if let TagToken(Tag { kind: StartTag, name: atom!(svg), .. }) = *token {
                return false;
            }
        }

        if html_integration_point(name.clone()) {
            match *token {
                CharacterTokens(..) => return false,
                TagToken(Tag { kind: StartTag, .. }) => return false,
                _ => (),
            }
        }

        true
    }
    //§ END

    fn enter_foreign(&mut self, mut tag: Tag, ns: Namespace) -> ProcessResult {
        match ns {
            ns!(MathML) => self.adjust_mathml_attributes(&mut tag),
            ns!(SVG) => self.adjust_svg_attributes(&mut tag),
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
            atom!(altglyph) => *name = atom!(altGlyph),
            atom!(altglyphdef) => *name = atom!(altGlyphDef),
            atom!(altglyphitem) => *name = atom!(altGlyphItem),
            atom!(animatecolor) => *name = atom!(animateColor),
            atom!(animatemotion) => *name = atom!(animateMotion),
            atom!(animatetransform) => *name = atom!(animateTransform),
            atom!(clippath) => *name = atom!(clipPath),
            atom!(feblend) => *name = atom!(feBlend),
            atom!(fecolormatrix) => *name = atom!(feColorMatrix),
            atom!(fecomponenttransfer) => *name = atom!(feComponentTransfer),
            atom!(fecomposite) => *name = atom!(feComposite),
            atom!(feconvolvematrix) => *name = atom!(feConvolveMatrix),
            atom!(fediffuselighting) => *name = atom!(feDiffuseLighting),
            atom!(fedisplacementmap) => *name = atom!(feDisplacementMap),
            atom!(fedistantlight) => *name = atom!(feDistantLight),
            atom!(fedropshadow) => *name = atom!(feDropShadow),
            atom!(feflood) => *name = atom!(feFlood),
            atom!(fefunca) => *name = atom!(feFuncA),
            atom!(fefuncb) => *name = atom!(feFuncB),
            atom!(fefuncg) => *name = atom!(feFuncG),
            atom!(fefuncr) => *name = atom!(feFuncR),
            atom!(fegaussianblur) => *name = atom!(feGaussianBlur),
            atom!(feimage) => *name = atom!(feImage),
            atom!(femerge) => *name = atom!(feMerge),
            atom!(femergenode) => *name = atom!(feMergeNode),
            atom!(femorphology) => *name = atom!(feMorphology),
            atom!(feoffset) => *name = atom!(feOffset),
            atom!(fepointlight) => *name = atom!(fePointLight),
            atom!(fespecularlighting) => *name = atom!(feSpecularLighting),
            atom!(fespotlight) => *name = atom!(feSpotLight),
            atom!(fetile) => *name = atom!(feTile),
            atom!(feturbulence) => *name = atom!(feTurbulence),
            atom!(foreignobject) => *name = atom!(foreignObject),
            atom!(glyphref) => *name = atom!(glyphRef),
            atom!(lineargradient) => *name = atom!(linearGradient),
            atom!(radialgradient) => *name = atom!(radialGradient),
            atom!(textpath) => *name = atom!(textPath),
            _ => (),
        }
    }

    fn adjust_attributes<F>(&mut self, tag: &mut Tag, mut map: F)
        where F: FnMut(Atom) -> Option<QualName>,
    {
        for &mut Attribute { ref mut name, .. } in &mut tag.attrs {
            if let Some(replacement) = map(name.local.clone()) {
                *name = replacement;
            }
        }
    }

    fn adjust_svg_attributes(&mut self, tag: &mut Tag) {
        self.adjust_attributes(tag, |k| match k {
            atom!(attributename) => Some(qualname!("", attributeName)),
            atom!(attributetype) => Some(qualname!("", attributeType)),
            atom!(basefrequency) => Some(qualname!("", baseFrequency)),
            atom!(baseprofile) => Some(qualname!("", baseProfile)),
            atom!(calcmode) => Some(qualname!("", calcMode)),
            atom!(clippathunits) => Some(qualname!("", clipPathUnits)),
            atom!(diffuseconstant) => Some(qualname!("", diffuseConstant)),
            atom!(edgemode) => Some(qualname!("", edgeMode)),
            atom!(filterunits) => Some(qualname!("", filterUnits)),
            atom!(glyphref) => Some(qualname!("", glyphRef)),
            atom!(gradienttransform) => Some(qualname!("", gradientTransform)),
            atom!(gradientunits) => Some(qualname!("", gradientUnits)),
            atom!(kernelmatrix) => Some(qualname!("", kernelMatrix)),
            atom!(kernelunitlength) => Some(qualname!("", kernelUnitLength)),
            atom!(keypoints) => Some(qualname!("", keyPoints)),
            atom!(keysplines) => Some(qualname!("", keySplines)),
            atom!(keytimes) => Some(qualname!("", keyTimes)),
            atom!(lengthadjust) => Some(qualname!("", lengthAdjust)),
            atom!(limitingconeangle) => Some(qualname!("", limitingConeAngle)),
            atom!(markerheight) => Some(qualname!("", markerHeight)),
            atom!(markerunits) => Some(qualname!("", markerUnits)),
            atom!(markerwidth) => Some(qualname!("", markerWidth)),
            atom!(maskcontentunits) => Some(qualname!("", maskContentUnits)),
            atom!(maskunits) => Some(qualname!("", maskUnits)),
            atom!(numoctaves) => Some(qualname!("", numOctaves)),
            atom!(pathlength) => Some(qualname!("", pathLength)),
            atom!(patterncontentunits) => Some(qualname!("", patternContentUnits)),
            atom!(patterntransform) => Some(qualname!("", patternTransform)),
            atom!(patternunits) => Some(qualname!("", patternUnits)),
            atom!(pointsatx) => Some(qualname!("", pointsAtX)),
            atom!(pointsaty) => Some(qualname!("", pointsAtY)),
            atom!(pointsatz) => Some(qualname!("", pointsAtZ)),
            atom!(preservealpha) => Some(qualname!("", preserveAlpha)),
            atom!(preserveaspectratio) => Some(qualname!("", preserveAspectRatio)),
            atom!(primitiveunits) => Some(qualname!("", primitiveUnits)),
            atom!(refx) => Some(qualname!("", refX)),
            atom!(refy) => Some(qualname!("", refY)),
            atom!(repeatcount) => Some(qualname!("", repeatCount)),
            atom!(repeatdur) => Some(qualname!("", repeatDur)),
            atom!(requiredextensions) => Some(qualname!("", requiredExtensions)),
            atom!(requiredfeatures) => Some(qualname!("", requiredFeatures)),
            atom!(specularconstant) => Some(qualname!("", specularConstant)),
            atom!(specularexponent) => Some(qualname!("", specularExponent)),
            atom!(spreadmethod) => Some(qualname!("", spreadMethod)),
            atom!(startoffset) => Some(qualname!("", startOffset)),
            atom!(stddeviation) => Some(qualname!("", stdDeviation)),
            atom!(stitchtiles) => Some(qualname!("", stitchTiles)),
            atom!(surfacescale) => Some(qualname!("", surfaceScale)),
            atom!(systemlanguage) => Some(qualname!("", systemLanguage)),
            atom!(tablevalues) => Some(qualname!("", tableValues)),
            atom!(targetx) => Some(qualname!("", targetX)),
            atom!(targety) => Some(qualname!("", targetY)),
            atom!(textlength) => Some(qualname!("", textLength)),
            atom!(viewbox) => Some(qualname!("", viewBox)),
            atom!(viewtarget) => Some(qualname!("", viewTarget)),
            atom!(xchannelselector) => Some(qualname!("", xChannelSelector)),
            atom!(ychannelselector) => Some(qualname!("", yChannelSelector)),
            atom!(zoomandpan) => Some(qualname!("", zoomAndPan)),
            _ => None,
        });
    }

    fn adjust_mathml_attributes(&mut self, tag: &mut Tag) {
        self.adjust_attributes(tag, |k| match k {
            atom!(definitionurl) => Some(qualname!("", definitionURL)),
            _ => None,
        });
    }

    fn adjust_foreign_attributes(&mut self, tag: &mut Tag) {
        self.adjust_attributes(tag, |k| match k {
            atom!("xlink:actuate") => Some(qualname!(XLink, actuate)),
            atom!("xlink:arcrole") => Some(qualname!(XLink, arcrole)),
            atom!("xlink:href") => Some(qualname!(XLink, href)),
            atom!("xlink:role") => Some(qualname!(XLink, role)),
            atom!("xlink:show") => Some(qualname!(XLink, show)),
            atom!("xlink:title") => Some(qualname!(XLink, title)),
            atom!("xlink:type") => Some(qualname!(XLink, "type")),
            atom!("xml:base") => Some(qualname!(XML, base)),
            atom!("xml:lang") => Some(qualname!(XML, lang)),
            atom!("xml:space") => Some(qualname!(XML, space)),
            atom!("xmlns") => Some(qualname!(XMLNS, xmlns)),
            atom!("xmlns:xlink") => Some(qualname!(XMLNS, xlink)),
            _ => None,
        });
    }

    fn foreign_start_tag(&mut self, mut tag: Tag) -> ProcessResult {
        let cur = self.sink.elem_name(&self.adjusted_current_node());
        match cur.ns {
            ns!(MathML) => self.adjust_mathml_attributes(&mut tag),
            ns!(SVG) => {
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
}

pub trait XmlTreeBuilderActions<Handle> {
    fn current_node(&self) -> Handle;
    fn insert_appropriately(&mut self, child: NodeOrText<Handle>);
    fn insert_tag(&mut self, tag: XTag) -> XmlProcessResult;
    fn append_tag(&mut self, tag: XTag) -> XmlProcessResult;
    fn append_tag_to_doc(&mut self, tag: XTag) -> Handle;
    fn add_to_open_elems(&mut self, el: Handle) -> XmlProcessResult;
    fn append_comment_to_doc(&mut self, comment: String) -> XmlProcessResult;
    fn append_comment_to_tag(&mut self, text: String) -> XmlProcessResult;
    fn append_pi_to_doc(&mut self, pi: XPi) -> XmlProcessResult;
    fn append_pi_to_tag(&mut self, pi: XPi) -> XmlProcessResult;
    fn append_text(&mut self, chars: String) -> XmlProcessResult;
    fn tag_in_open_elems(&self, tag: &XTag) -> bool;
    fn pop_until<TagSet>(&mut self, pred: TagSet) where TagSet: Fn(QualName) -> bool;
    fn current_node_in<TagSet>(&self, set: TagSet) -> bool where TagSet: Fn(QualName) -> bool;
    fn close_tag(&mut self, tag: XTag) -> XmlProcessResult;
    fn no_open_elems(&self) -> bool;
    fn pop(&mut self) -> Handle ;
    fn stop_parsing(&mut self) -> XmlProcessResult;
}

#[doc(hidden)]
impl<Handle, Sink> XmlTreeBuilderActions<Handle>
    for super::XmlTreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{

    fn current_node(&self) -> Handle {
        self.open_elems.last().expect("no current element").clone()
    }

    fn insert_appropriately(&mut self, child: NodeOrText<Handle>){
        let target = self.current_node();
        self.sink.append(target, child);
    }

    fn insert_tag(&mut self, tag: XTag) -> XmlProcessResult {
        let child = self.sink.create_element(QualName::new(ns!(HTML),
            tag.name), tag.attrs);
        self.insert_appropriately(AppendNode(child.clone()));
        self.add_to_open_elems(child)
    }

    fn append_tag(&mut self, tag: XTag) -> XmlProcessResult {
        let child = self.sink.create_element(QualName::new(ns!(HTML),
            tag.name), tag.attrs);
        self.insert_appropriately(AppendNode(child));
        XDone
    }

    fn append_tag_to_doc(&mut self, tag: XTag) -> Handle {
        let root = self.doc_handle.clone();
        let child = self.sink.create_element(QualName::new(ns!(HTML),
            tag.name), tag.attrs);

        self.sink.append(root, AppendNode(child.clone()));
        child
    }

    fn add_to_open_elems(&mut self, el: Handle) -> XmlProcessResult {
        self.open_elems.push(el);

        //FIXME remove this on final commit
        println!("After add to open elems there are {} open elems", self.open_elems.len());
        XDone
    }

    fn append_comment_to_doc(&mut self, text: String) -> XmlProcessResult {
        let target = self.doc_handle.clone();
        let comment = self.sink.create_comment(text);
        self.sink.append(target, AppendNode(comment));
        XDone
    }

    fn append_comment_to_tag(&mut self, text: String) -> XmlProcessResult {
        let target = self.current_node();
        let comment = self.sink.create_comment(text);
        self.sink.append(target, AppendNode(comment));
        XDone
    }

    fn append_pi_to_doc(&mut self, pi: XPi) -> XmlProcessResult {
        let target = self.doc_handle.clone();
        let pi = self.sink.create_pi(pi.target, pi.data);
        self.sink.append(target, AppendNode(pi));
        XDone
    }

    fn append_pi_to_tag(&mut self, pi: XPi) -> XmlProcessResult {
        let target = self.current_node();
        let pi = self.sink.create_pi(pi.target, pi.data);
        self.sink.append(target, AppendNode(pi));
        XDone
    }


    fn append_text(&mut self, chars: String)
        -> XmlProcessResult {
        self.insert_appropriately(AppendText(chars));
        XDone
    }

    fn tag_in_open_elems(&self, tag: &XTag) -> bool {
        self.open_elems
            .iter()
            .any(|a| self.sink.elem_name(a) == QualName::new(ns!(HTML), tag.name.clone()))
    }

    // Pop elements until an element from the set has been popped.  Returns the
    // number of elements popped.
    fn pop_until<P>(&mut self, pred: P)
        where P: Fn(QualName) -> bool
    {
        loop {
            if self.current_node_in(|x| pred(x)) {
                break;
            }
            self.open_elems.pop();
        }
    }

    fn current_node_in<TagSet>(&self, set: TagSet) -> bool
        where TagSet: Fn(QualName) -> bool
    {
        set(self.sink.elem_name(&self.current_node()))
    }

    fn close_tag(&mut self, tag: XTag) -> XmlProcessResult {
        println!("Close tag: current_node.name {:?} \n Current tag {:?}",
                 self.sink.elem_name(&self.current_node()), &tag.name);
        if &self.sink.elem_name(&self.current_node()).local != &tag.name {
            self.sink.parse_error(Borrowed("Current node doesn't match tag"));
        }
        // FIXME remove this part after debug
        let is_closed = self.tag_in_open_elems(&tag);
        println!("Close tag {:?}", is_closed);

        if(is_closed) {
            // FIXME: Real namespace resolution
            self.pop_until(|p| p == QualName::new(ns!(HTML), tag.name.clone()));
            self.pop();
        }
        XDone
    }

    fn no_open_elems(&self) -> bool {
        self.open_elems.is_empty()
    }

    fn pop(&mut self) -> Handle {
        self.open_elems.pop().expect("no current element")
    }

    fn stop_parsing(&mut self) -> XmlProcessResult {
        h5e_warn!("stop_parsing for XML5 not implemented, full speed ahead!");
        XDone
    }
}
