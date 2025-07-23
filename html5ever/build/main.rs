// Copyright 2014-2025 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Generates a DAFSA at compile time for resolving named character references

use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap, VecDeque},
    env, fmt,
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::Path,
};

use serde::Deserialize;

struct Node {
    /// One edge per ASCII character
    edges: [Option<usize>; 128],
    is_terminal: bool,
    /// Represents the number of terminal nodes within this node's subtree.
    ///
    /// This is needed for minimal perfect hashing within the DAFSA.
    num_nodes: usize,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("is_terminal", &self.is_terminal)
            .field("num_nodes", &self.num_nodes)
            .finish()
    }
}

struct Transition {
    from: usize,
    /// ASCII character
    character: u8,
    to: usize,
}

struct DafsaBuilder {
    previous_word: String,
    unchecked_transitions: Vec<Transition>,
    minimized_nodes: Vec<usize>,
    /// First node is always the root node.
    nodes: Vec<Node>,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            is_terminal: false,
            edges: [const { None }; 128],
            num_nodes: 0,
        }
    }
}

impl Default for DafsaBuilder {
    fn default() -> Self {
        Self {
            previous_word: String::default(),
            unchecked_transitions: Vec::default(),
            minimized_nodes: Vec::default(),
            nodes: vec![Default::default()],
        }
    }
}

impl DafsaBuilder {
    fn allocate_node(&mut self) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node::default());
        index
    }

    /// Insert a new word into the DAFSA, while maintaining invariants.
    ///
    /// This implements the algorithm described in <https://stevehanov.ca/blog/?id=115>.
    fn insert(&mut self, new_word: String) {
        assert!(
            new_word > self.previous_word,
            "Words must be inserted in order"
        );

        // We can operate on bytes here, because all named character references are
        // restriced to ASCII code points.
        let common_prefix_length = new_word
            .bytes()
            .zip(self.previous_word.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or(new_word.len().min(self.previous_word.len()));

        self.minimize(common_prefix_length);

        // add the suffix, starting from the correct node mid-way through the graph
        let mut node_handle = self
            .unchecked_transitions
            .last()
            .map(|transition| transition.to)
            .unwrap_or_default();

        let remaining_code_points = &new_word.as_bytes()[common_prefix_length..];
        for code_point in remaining_code_points {
            assert!(
                self.nodes[node_handle].edges[*code_point as usize].is_none(),
                "Should have found a longer common prefix"
            );

            let new_child_handle = self.allocate_node();
            self.nodes[node_handle].edges[*code_point as usize] = Some(new_child_handle);
            self.unchecked_transitions.push(Transition {
                from: node_handle,
                character: *code_point,
                to: new_child_handle,
            });
            node_handle = new_child_handle;
        }

        self.nodes[node_handle].is_terminal = true;
        self.previous_word = new_word;
    }

    /// Check the uncheckedNodes for redundant nodes, proceeding from last
    /// one down to the common prefix size. Then truncate the list at that
    /// point.
    fn minimize(&mut self, down_to: usize) {
        while self.unchecked_transitions.len() > down_to {
            let transition = self.unchecked_transitions.pop().unwrap();

            if let Some(equal_minimized_node) = self
                .minimized_nodes
                .iter()
                .find(|minimized_node| self.are_subtrees_equal(**minimized_node, transition.to))
            {
                self.nodes[transition.from].edges[transition.character as usize] =
                    Some(*equal_minimized_node);
            } else {
                self.minimized_nodes.push(transition.to);
            }
        }
    }

    fn finish(&mut self) {
        self.minimize(0);
    }

    fn are_subtrees_equal(&self, first: usize, second: usize) -> bool {
        if first == second {
            return true;
        }

        let first = &self.nodes[first];
        let second = &self.nodes[second];

        if first.is_terminal != second.is_terminal {
            return false;
        }

        // First check if exactly the same edges are present
        if first
            .edges
            .iter()
            .zip(second.edges.iter())
            .any(|(a, b)| a.is_some() != b.is_some())
        {
            return false;
        }

        // The check whether the subtrees at each edge are equal.
        first
            .edges
            .iter()
            .zip(second.edges.iter())
            .filter_map(|(first_edge, second_edge)| first_edge.zip(*second_edge))
            .all(|(first_edge, second_edge)| self.are_subtrees_equal(first_edge, second_edge))
    }

    fn compute_numbers_for(&mut self, index: usize) {
        if self.nodes[index].num_nodes != 0 {
            // We already computed the unique value for this node
            return;
        }

        self.nodes[index].num_nodes += self.nodes[index].is_terminal as usize;
        for edge in self.nodes[index].edges {
            let Some(edge) = edge else {
                continue;
            };

            self.compute_numbers_for(edge);
            self.nodes[index].num_nodes += self.nodes[edge].num_nodes;
        }
    }

    /// Computes all numbers needed for minimal perfect hashing
    fn compute_numbers(&mut self) {
        self.compute_numbers_for(0);
    }

    /// Returns the perfect hash value for the input, or `None` if
    /// the input was not in the input set.
    ///
    /// Hashing is done by computing a prefix sum over `Node::num_nodes` at
    /// every step of the traversal.
    fn get_unique_index(&self, input: &str) -> Option<usize> {
        assert!(input.is_ascii());

        let mut index = 0;
        let mut current = &self.nodes[0];
        for code_point in input.as_bytes() {
            let next_node = current.edges[*code_point as usize]?;

            for edge in &current.edges[..*code_point as usize] {
                let Some(edge) = edge else {
                    continue;
                };
                index += self.nodes[*edge as usize].num_nodes;
            }

            current = &self.nodes[next_node];
            if current.is_terminal {
                index += 1;
            }
        }

        debug_assert!(
            current.is_terminal,
            "Traversing {input:?} did not end at a terminal node"
        );
        Some(index)
    }
}

impl Node {
    fn edges(&self) -> impl Iterator<Item = usize> + use<'_> {
        self.edges.iter().filter_map(|edge| *edge)
    }
}

#[derive(Deserialize)]
struct NamedEntity {
    codepoints: Vec<u32>,
    #[allow(dead_code)]
    characters: String,
}

fn main() {
    // Parse the list of named entities from https://html.spec.whatwg.org/entities.json
    let input_file = BufReader::new(File::open("build/entities.json").unwrap());
    let named_entities: BTreeMap<String, NamedEntity> =
        serde_json::from_reader(input_file).unwrap();

    // Build the DAFSA of all named references
    let mut dafsa_builder = DafsaBuilder::default();
    for (name, _) in &named_entities {
        let name = name.strip_prefix('&').unwrap();
        dafsa_builder.insert(name.to_string());
    }
    dafsa_builder.finish();
    dafsa_builder.compute_numbers();

    // Assert that there are no collisions in the perfect hash map, as a sanity check.
    let mut seen_indices = HashMap::new();
    for (name, _) in &named_entities {
        let name = name.strip_prefix('&').unwrap();

        let previous_value = seen_indices.insert(
            dafsa_builder.get_unique_index(name).unwrap(),
            name.to_owned(),
        );
        assert!(
            previous_value.is_none(),
            "Collision on {name} with {previous_value:?}"
        );
    }

    // Generate implementation
    let mut stack = VecDeque::new();
    stack.push_back(0); // Initially we only know about the root node

    let mut next_available_index = dafsa_builder.nodes[0].edges().count() + 1;

    // Maps from a node to the index of its first child
    let mut first_child_index = HashMap::new();

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let destination_path = Path::new(&out_dir).join("named_entities_graph.rs");
    let mut result = BufWriter::new(File::create(destination_path).unwrap());
    writeln!(
        &mut result,
        "pub(crate) const DAFSA_NODES: [Node; 3872] = ["
    )
    .unwrap();

    // Define all nodes by traversing the DAFSA graph
    write!(&mut result, "Node::new(0, 0, false, true, 1),").unwrap();
    while let Some(handle) = stack.pop_front() {
        let node = &dafsa_builder.nodes[handle];
        let num_children = node.edges().count();

        let mut child_index = 0;
        for (code_point, child_handle) in node.edges.iter().enumerate() {
            let Some(child_handle) = *child_handle else {
                continue;
            };

            let child = &dafsa_builder.nodes[child_handle];
            let is_last_child = child_index == num_children - 1;

            let first_child_index = match first_child_index.entry(child_handle) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => {
                    let descendants = child.edges().count();

                    stack.push_back(child_handle);
                    if descendants > 0 {
                        let first_child = next_available_index;
                        entry.insert(first_child);
                        next_available_index += descendants;
                        first_child
                    } else {
                        // If the node has no descendants then we set the first child index to
                        // zero, as the root node cannot be a descendant of anybody (due to the graph being acyclic).
                        0
                    }
                },
            };

            write!(
                &mut result,
                "Node::new({code_point}, {hash_value}, {is_terminal}, {is_last_child}, {first_child_index}),",
                is_terminal = child.is_terminal,
                hash_value = child.num_nodes
            )
            .unwrap();
            child_index += 1;
        }
    }
    writeln!(&mut result, "];").unwrap();

    // Define the lookup table for the PHF values
    let num_entities = named_entities.len();
    let mut references: Vec<(u32, u32)> = vec![Default::default(); named_entities.len() + 1];
    for (name, entity) in &named_entities {
        let name = name.strip_prefix('&').unwrap();
        let unique_index = dafsa_builder.get_unique_index(name).unwrap();

        // For safety, ensure that there are no collisions
        assert_eq!(references[unique_index].0, 0);

        references[unique_index] = (
            entity.codepoints[0],
            entity.codepoints.get(1).copied().unwrap_or_default() as u32,
        );
    }

    writeln!(
        &mut result,
        "pub(crate) const REFERENCES: [(u32, u32); {}] = {references:?};",
        num_entities + 1
    )
    .unwrap();
}
