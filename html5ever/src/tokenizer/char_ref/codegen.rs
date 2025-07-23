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

#[derive(Clone, Copy, Debug)]
pub(crate) struct Node {
    first_child_index: usize,
    code_point: u8,
    is_last_child: bool,
    is_terminal: bool,
    num_nodes: u8,
}

impl Node {
    pub(crate) const fn code_point(&self) -> u8 {
        self.code_point
    }

    pub(crate) const fn num_nodes(&self) -> usize {
        self.num_nodes as usize
    }

    pub(crate) const fn is_terminal(&self) -> bool {
        self.is_terminal
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

                if node.is_last_child {
                    self.done = true;
                }

                Some(node)
            }
        }

        ChildIterator {
            index: self.first_child_index,
            done: self.first_child_index == 0,
        }
    }
}

// fn compute_unique_index(input: &str) -> Option<usize> {
//     debug_assert!(input.is_ascii());

//     let mut index = 0;
//     let mut current = &DAFSA_NODES[0];
//     for code_point in input.as_bytes() {
//         let mut next_node = None;
//         for child in current.children() {
//             if child.code_point == *code_point {
//                 next_node = Some(child);
//                 break;
//             } else {
//                 index += child.num_nodes as usize;
//             }
//         }

//         current = next_node?;

//         if current.is_terminal {
//             index += 1;
//         }
//     }

//     if current.is_terminal {
//         Some(index)
//     } else {
//         None
//     }
// }

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
