// Copyright 2014-2025 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::tokenizer::CharRef;

include!(concat!(env!("OUT_DIR"), "/named_entities_graph.rs"));

/// A single node in the DAFSA.
///
/// For memory efficiency reasons, this is packed in 32 bits. The memory representation is as follows:
/// * 8 bits: code point
/// * 8 bits: hash value
#[derive(Clone, Copy, Debug)]
pub(crate) struct Node(u32);

impl Node {
    const IS_TERMINAL: u32 = 1 << 15;
    const IS_LAST_CHILD: u32 = 1 << 14;

    pub(crate) const fn new(
        code_point: u8,
        hash_value: u8,
        is_terminal: bool,
        is_last_child: bool,
        first_child_index: u16,
    ) -> Self {
        let mut value = 0;
        value |= (code_point as u32) << 24;
        value |= (hash_value as u32) << 16;

        if is_terminal {
            value |= Self::IS_TERMINAL;
        }

        if is_last_child {
            value |= Self::IS_LAST_CHILD;
        }

        assert!(first_child_index <= 0xFFF);

        value |= first_child_index as u32;

        Self(value)
    }

    pub(crate) const fn code_point(&self) -> u8 {
        (self.0 >> 24) as u8
    }

    pub(crate) const fn hash_value(&self) -> usize {
        ((self.0 >> 16) & 0xFF) as usize
    }

    pub(crate) const fn is_terminal(&self) -> bool {
        (self.0 & Self::IS_TERMINAL) != 0
    }

    const fn is_last_child(&self) -> bool {
        (self.0 & Self::IS_LAST_CHILD) != 0
    }

    const fn first_child_index(&self) -> u16 {
        (self.0 & 0xFFF) as u16
    }

    pub(crate) fn children(&self) -> impl Iterator<Item = &'static Node> {
        struct ChildIterator {
            index: usize,
            done: bool,
        }

        impl Iterator for ChildIterator {
            type Item = &'static Node;

            fn next(&mut self) -> Option<Self::Item> {
                if self.done {
                    return None;
                }
                let node = &DAFSA_NODES[self.index];
                self.index += 1;

                if node.is_last_child() {
                    self.done = true;
                }

                Some(node)
            }
        }

        let first_child_index = self.first_child_index();
        ChildIterator {
            index: first_child_index as usize,
            done: first_child_index == 0,
        }
    }
}

pub(crate) fn resolve_unique_hash_value(value: usize) -> CharRef {
    let (first, second) = REFERENCES[value];

    let num_chars = if second == 0 { 1 } else { 2 };

    CharRef {
        chars: [
            char::from_u32(first).unwrap(),
            char::from_u32(second).unwrap(),
        ],
        num_chars,
    }
}
