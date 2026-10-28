// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The stack of open elements.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::ops::Deref;

use crate::tree_builder::tag_sets::*;
use crate::{ExpandedName, LocalName};
use markup5ever::ns;

/// Sets of elements that the tree builder looks for in the stack of open elements.
///
/// The stack keeps the positions of the open elements in each of these sets, so that finding
/// the topmost one doesn't need a walk down the stack.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TrackedSet {
    /// <https://html.spec.whatwg.org/multipage/#has-an-element-in-scope>
    DefaultScope,
    /// <https://html.spec.whatwg.org/multipage/#has-an-element-in-list-item-scope>
    ListItemScope,
    /// <https://html.spec.whatwg.org/multipage/#has-an-element-in-button-scope>
    ButtonScope,
    /// <https://html.spec.whatwg.org/multipage/#has-an-element-in-table-scope>
    TableScope,
    /// The special elements, where the "any other end tag" steps in body stop.
    Special,
    /// The special elements other than address, div and p, where the steps for a li, dd or dt
    /// start tag in body stop.
    SpecialExceptAddressDivP,
    /// The elements where "reset the insertion mode appropriately" stops.
    ResetInsertionMode,
    /// The elements that are a parse error when they are still open at the end of the body.
    NotBodyEndOk,
    /// HTML elements, where the steps for an end tag in foreign content stop.
    Html,
}

const TRACKED_SETS: [TrackedSet; 9] = [
    TrackedSet::DefaultScope,
    TrackedSet::ListItemScope,
    TrackedSet::ButtonScope,
    TrackedSet::TableScope,
    TrackedSet::Special,
    TrackedSet::SpecialExceptAddressDivP,
    TrackedSet::ResetInsertionMode,
    TrackedSet::NotBodyEndOk,
    TrackedSet::Html,
];

impl TrackedSet {
    fn contains(self, name: ExpandedName) -> bool {
        match self {
            TrackedSet::DefaultScope => default_scope(name),
            TrackedSet::ListItemScope => list_item_scope(name),
            TrackedSet::ButtonScope => button_scope(name),
            TrackedSet::TableScope => table_scope(name),
            TrackedSet::Special => special_tag(name),
            TrackedSet::SpecialExceptAddressDivP => special_except_address_div_p(name),
            TrackedSet::ResetInsertionMode => reset_insertion_mode_stop(name),
            TrackedSet::NotBodyEndOk => !body_end_ok(name),
            TrackedSet::Html => *name.ns == ns!(html),
        }
    }
}

/// Hashes a `LocalName` by the hash that string_cache keeps for each atom, mixed so that short
/// atoms, whose hash is their inline representation, spread over the table too.
#[derive(Default)]
struct AtomHasher(u64);

impl Hasher for AtomHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }

    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }

    fn finish(&self) -> u64 {
        self.0 ^ (self.0 >> 32)
    }
}

type AtomMap<V> = HashMap<LocalName, V, BuildHasherDefault<AtomHasher>>;

/// What the stack knows about one of its elements.
#[derive(Clone, Copy, PartialEq)]
struct Entry {
    /// Index into `OpenElems::positions_by_name`.
    name: u32,
    /// The tracked sets the element is in, as a bit set indexed by `TrackedSet as usize`.
    sets: u16,
}

/// The stack of open elements, most recently added at the end.
///
/// Besides the elements themselves, it keeps the stack positions of the open elements with each
/// name and of those in each [`TrackedSet`], lowest first. "Has an element in scope" then compares
/// the topmost element with that name to the topmost element that bounds the scope, instead of
/// walking down the stack, so that the cost of a token doesn't grow with the nesting depth.
///
/// Pushing and popping update these positions in constant time. Removing an element below the top
/// re-indexes the elements above it, and moving one up re-indexes the elements it moves past.
pub(crate) struct OpenElems<Handle> {
    elems: Vec<Handle>,
    entries: Vec<Entry>,
    /// The entries of HTML elements, by name.
    html_names: AtomMap<Entry>,
    /// Indices into `positions_by_name` for the names of foreign elements, lowercased, because
    /// end tags in foreign content match them ASCII case-insensitively.
    foreign_names: AtomMap<u32>,
    /// Positions are stored as `u32` to halve the memory per open element; `push` refuses a
    /// stack deeper than `u32::MAX`.
    positions_by_name: Vec<Vec<u32>>,
    positions_by_set: [Vec<u32>; TRACKED_SETS.len()],
}

/// A list of stack positions, lowest first.
#[derive(Clone, Copy)]
pub(crate) struct Positions<'a>(&'a [u32]);

impl<'a> Positions<'a> {
    pub(crate) fn first(self) -> Option<usize> {
        self.0.first().map(|&p| p as usize)
    }

    pub(crate) fn last(self) -> Option<usize> {
        self.0.last().map(|&p| p as usize)
    }

    pub(crate) fn is_empty(self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn iter(self) -> impl DoubleEndedIterator<Item = usize> + 'a {
        self.0.iter().map(|&p| p as usize)
    }
}

impl<Handle> Default for OpenElems<Handle> {
    fn default() -> Self {
        OpenElems {
            elems: vec![],
            entries: vec![],
            html_names: AtomMap::default(),
            foreign_names: AtomMap::default(),
            positions_by_name: vec![],
            positions_by_set: Default::default(),
        }
    }
}

impl<Handle> Deref for OpenElems<Handle> {
    type Target = [Handle];

    fn deref(&self) -> &[Handle] {
        &self.elems
    }
}

impl<Handle> OpenElems<Handle> {
    pub(crate) fn push(&mut self, elem: Handle, name: ExpandedName) {
        let entry = self.entry_for(name);
        assert!(
            self.elems.len() < u32::MAX as usize,
            "stack of open elements too deep"
        );
        self.elems.push(elem);
        self.entries.push(entry);
        self.index_from(self.entries.len() - 1);
    }

    pub(crate) fn pop(&mut self) -> Option<Handle> {
        let elem = self.elems.pop()?;
        self.unindex_from(self.entries.len() - 1);
        self.entries.pop();
        Some(elem)
    }

    pub(crate) fn truncate(&mut self, len: usize) {
        while self.elems.len() > len {
            self.pop();
        }
    }

    pub(crate) fn remove(&mut self, index: usize) -> Handle {
        self.unindex_from(index);
        self.entries.remove(index);
        let elem = self.elems.remove(index);
        self.index_from(index);
        elem
    }

    /// Move the element at `from` up to `to`, which moves the elements above it, up to and
    /// including `to`, down by one.
    pub(crate) fn move_up(&mut self, from: usize, to: usize) {
        // Only the lists with a position from `from` to `to` change, and only there.
        let moved = self.entries[from];
        let mut names: Vec<u32> = self.entries[from..=to].iter().map(|e| e.name).collect();
        names.sort_unstable();
        names.dedup();
        for name in names {
            let positions = &mut self.positions_by_name[name as usize];
            move_position_up(positions, from, to, name == moved.name);
        }
        let sets = self.entries[from..=to]
            .iter()
            .fold(0, |sets, e| sets | e.sets);
        for set in TRACKED_SETS {
            let bit = 1 << set as u16;
            if sets & bit != 0 {
                let positions = &mut self.positions_by_set[set as usize];
                move_position_up(positions, from, to, moved.sets & bit != 0);
            }
        }
        self.entries[from..=to].rotate_left(1);
        self.elems[from..=to].rotate_left(1);
    }

    pub(crate) fn replace(&mut self, index: usize, elem: Handle, name: ExpandedName) {
        let entry = self.entry_for(name);
        // The adoption agency algorithm replaces an element with a new one with the same name,
        // which leaves the positions as they are.
        if entry != self.entries[index] {
            self.unindex_from(index);
            self.entries[index] = entry;
            self.index_from(index);
        }
        self.elems[index] = elem;
    }

    /// The stack positions of the open HTML elements named `name`, lowest first.
    pub(crate) fn positions_named(&self, name: &LocalName) -> Positions<'_> {
        Positions(match self.html_names.get(name) {
            Some(entry) => &self.positions_by_name[entry.name as usize],
            None => &[],
        })
    }

    /// The stack positions of the open foreign elements whose name is an ASCII case-insensitive
    /// match for `name`, lowest first.
    pub(crate) fn positions_foreign_named(&self, name: &LocalName) -> Positions<'_> {
        Positions(match self.foreign_names.get(&name.to_ascii_lowercase()) {
            Some(&id) => &self.positions_by_name[id as usize],
            None => &[],
        })
    }

    /// The stack positions of the open elements in `set`, lowest first.
    pub(crate) fn positions_in(&self, set: TrackedSet) -> Positions<'_> {
        Positions(&self.positions_by_set[set as usize])
    }

    /// Is the element at `position` in the scope that `scope` bounds? It is unless an element
    /// in `scope` is above it.
    pub(crate) fn in_scope(&self, scope: TrackedSet, position: usize) -> bool {
        self.positions_in(scope)
            .last()
            .is_none_or(|boundary| position >= boundary)
    }

    /// Is an HTML element named `name` in the scope that `scope` bounds?
    pub(crate) fn named_in_scope(&self, scope: TrackedSet, name: &LocalName) -> bool {
        self.positions_named(name)
            .last()
            .is_some_and(|position| self.in_scope(scope, position))
    }

    fn entry_for(&mut self, name: ExpandedName) -> Entry {
        if *name.ns == ns!(html) {
            // The sets an HTML element is in depend only on its local name, so they are worked
            // out once for each name.
            if let Some(&entry) = self.html_names.get(name.local) {
                return entry;
            }
            let entry = Entry {
                name: self.new_name(),
                sets: sets_of(name),
            };
            self.html_names.insert(name.local.clone(), entry);
            entry
        } else {
            let key = name.local.to_ascii_lowercase();
            let id = match self.foreign_names.get(&key) {
                Some(&id) => id,
                None => {
                    let id = self.new_name();
                    self.foreign_names.insert(key, id);
                    id
                },
            };
            Entry {
                name: id,
                sets: sets_of(name),
            }
        }
    }

    fn new_name(&mut self) -> u32 {
        // There are never more names than elements pushed, which `push` keeps below `u32::MAX`.
        self.positions_by_name.push(vec![]);
        (self.positions_by_name.len() - 1) as u32
    }

    /// Call `f` on each list of positions that an element with this entry is in.
    fn for_each_list(&mut self, entry: Entry, mut f: impl FnMut(&mut Vec<u32>)) {
        f(&mut self.positions_by_name[entry.name as usize]);
        for set in TRACKED_SETS {
            if entry.sets & (1 << set as u16) != 0 {
                f(&mut self.positions_by_set[set as usize]);
            }
        }
    }

    /// Record the positions of the elements from `start` to the top.
    fn index_from(&mut self, start: usize) {
        for position in start..self.entries.len() {
            self.for_each_list(self.entries[position], |positions| {
                positions.push(position as u32)
            });
        }
    }

    /// Forget the positions of the elements from `start` to the top. They are the highest
    /// recorded positions, so each is the last in its list.
    fn unindex_from(&mut self, start: usize) {
        for position in (start..self.entries.len()).rev() {
            self.for_each_list(self.entries[position], |positions| {
                positions.pop();
            });
        }
    }
}

/// Update a list of positions for `OpenElems::move_up`: the positions above `from`, up to and
/// including `to`, move down by one, and if the list has the moved element, `from` becomes `to`.
fn move_position_up(positions: &mut [u32], from: usize, to: usize, has_moved: bool) {
    let (from, to) = (from as u32, to as u32);
    let start = positions.partition_point(|&position| position < from);
    let end = positions.partition_point(|&position| position <= to);
    let mut changed = &mut positions[start..end];
    if has_moved {
        changed.rotate_left(1);
        let (moved, others) = changed.split_last_mut().unwrap();
        *moved = to;
        changed = others;
    }
    for position in changed {
        *position -= 1;
    }
}

fn sets_of(name: ExpandedName) -> u16 {
    let mut sets = 0;
    for set in TRACKED_SETS {
        if set.contains(name) {
            sets |= 1 << set as u16;
        }
    }
    sets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{local_name, Namespace, QualName};

    const NAMES: [(Namespace, &str); 14] = [
        (ns!(html), "html"),
        (ns!(html), "body"),
        (ns!(html), "div"),
        (ns!(html), "p"),
        (ns!(html), "li"),
        (ns!(html), "ul"),
        (ns!(html), "button"),
        (ns!(html), "table"),
        (ns!(html), "b"),
        (ns!(svg), "foreignObject"),
        (ns!(svg), "g"),
        (ns!(mathml), "mi"),
        (ns!(mathml), "g"),
        (ns!(mathml), "foreignobject"),
    ];

    fn qual_name(i: usize) -> QualName {
        let (ns, local) = NAMES[i].clone();
        QualName::new(None, ns, LocalName::from(local))
    }

    /// Check every recorded position against a walk down the stack.
    fn check(stack: &OpenElems<usize>) {
        let names: Vec<QualName> = stack.iter().map(|&i| qual_name(i)).collect();
        for set in TRACKED_SETS {
            let expected: Vec<usize> = (0..names.len())
                .filter(|&i| set.contains(names[i].expanded()))
                .collect();
            assert_eq!(
                stack.positions_in(set).iter().collect::<Vec<_>>(),
                expected,
                "{set:?}"
            );
        }
        for (_, local) in NAMES {
            let local = LocalName::from(local);
            let html: Vec<usize> = (0..names.len())
                .filter(|&i| names[i].ns == ns!(html) && names[i].local == local)
                .collect();
            assert_eq!(
                stack.positions_named(&local).iter().collect::<Vec<_>>(),
                html,
                "{local}"
            );
            // Foreign elements match whatever their namespace, and ignoring ASCII case.
            let foreign: Vec<usize> = (0..names.len())
                .filter(|&i| {
                    names[i].ns != ns!(html) && names[i].local.eq_ignore_ascii_case(&local)
                })
                .collect();
            assert_eq!(
                stack
                    .positions_foreign_named(&local)
                    .iter()
                    .collect::<Vec<_>>(),
                foreign,
                "{local}"
            );
            let uppercase = local.to_ascii_uppercase();
            assert_eq!(
                stack
                    .positions_foreign_named(&uppercase)
                    .iter()
                    .collect::<Vec<_>>(),
                foreign,
                "{local}"
            );
        }
    }

    #[test]
    fn positions_follow_the_stack() {
        let mut stack = OpenElems::default();
        // A fixed pseudo-random sequence of pushes, pops, removals, moves and replacements.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = |n: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % n as u64) as usize
        };
        for _ in 0..2000 {
            let name = next(NAMES.len());
            let len = stack.len();
            match next(6) {
                0 | 1 => stack.push(name, qual_name(name).expanded()),
                2 => {
                    stack.pop();
                },
                3 if len > 0 => {
                    stack.remove(next(len));
                },
                4 if len > 0 => {
                    let from = next(len);
                    stack.move_up(from, from + next(len - from));
                },
                5 if len > 0 => {
                    let index = next(len);
                    stack.replace(index, name, qual_name(name).expanded());
                },
                _ => {},
            }
            check(&stack);
        }
    }

    #[test]
    fn scope() {
        let mut stack = OpenElems::default();
        for name in [0, 1, 3, 6, 2] {
            stack.push(name, qual_name(name).expanded());
        }
        // html body p button div: the button bounds the button scope, not the default scope.
        assert!(stack.named_in_scope(TrackedSet::DefaultScope, &local_name!("p")));
        assert!(!stack.named_in_scope(TrackedSet::ButtonScope, &local_name!("p")));
        assert!(stack.named_in_scope(TrackedSet::ButtonScope, &local_name!("button")));
        assert!(!stack.named_in_scope(TrackedSet::DefaultScope, &local_name!("li")));
        stack.remove(3);
        assert!(stack.named_in_scope(TrackedSet::ButtonScope, &local_name!("p")));
    }
}
