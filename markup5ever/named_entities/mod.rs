// Copyright 2014-2025 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod codegen;

use codegen::{resolve_unique_hash_value, Node, DAFSA_NODES};
use super::{CharRef, Status};
use crate::buffer_queue::BufferQueue;
use crate::tendril::StrTendril;

use std::borrow::Cow;
use std::mem;

type EmitErrorFn = Fn(&str);

#[derive(Clone, Debug)]
pub(crate) struct Match {
    hash_value: usize,
    matched_text: StrTendril,
}

impl CharRef {
    pub const EMPTY: CharRef = CharRef {
        chars: ['\0', '\0'],
        num_chars: 0,
    };
}

#[derive(Clone, Debug)]
pub(crate) struct NamedReferenceTokenizerState {
    current_node: &'static Node,
    /// Contains all characters that the tokenizer has consumed since the last match.
    ///
    /// We can't always know whether these characters will be part of a named reference until
    /// we consume more. For example, `&not` is a valid named character reference, but it might continue
    /// to form `&notin`. When we have consumed `&noti` then only the `i` will be in the name buffer
    /// since it is the only character that needs to be flushed when no further reference is found.
    name_buffer: StrTendril,
    hash_value: usize,

    /// The last match (last terminal node) that we found during the traversal.
    last_match: Option<Match>,

    is_in_attribute: bool,
}

pub(crate) enum NamedReferenceTokenizationResult {
    /// Tokenization is complete.
    Success { reference: CharRef },
    /// The provided characters do not form a valid named reference and there is no
    /// valid continuation that would change that.
    Failed,
    /// The tokenizer expects more input.
    Continue,
}

impl NamedReferenceTokenizerState {
    pub(crate) fn new(is_in_attribute: bool) -> Self {
        Self {
            current_node: &DAFSA_NODES[0],
            name_buffer: Default::default(),
            hash_value: Default::default(),
            last_match: None,
            is_in_attribute,
        }
    }

    fn feed_character(
        &mut self,
        c: char,
        error_callback: EmitErrorFn,
        input: &BufferQueue,
    ) -> NamedReferenceTokenizationResult {
        self.name_buffer.push_char(c);
        if !c.is_ascii_alphanumeric() && c != ';' {
            return self.did_find_invalid_character(error_callback, input);
        }

        let code_point = c as u32 as u8;
        let mut next_node = None;
        for child in self.current_node.children() {
            if child.code_point() == code_point {
                next_node = Some(child);
                break;
            } else {
                self.hash_value += child.hash_value() as usize;
            }
        }

        let Some(next_node) = next_node else {
            return self.did_find_invalid_character(error_callback, input);
        };

        self.current_node = next_node;

        if self.current_node.is_terminal() {
            self.hash_value += 1;
            self.last_match = Some(Match {
                hash_value: self.hash_value,
                matched_text: mem::take(&mut self.name_buffer),
            });
        }

        NamedReferenceTokenizationResult::Continue
    }

    fn did_find_invalid_character(
        &mut self,
        error_callback: EmitErrorFn,
        input: &BufferQueue,
    ) -> NamedReferenceTokenizationResult {
        if let Some(last_match) = self.last_match.take() {
            input.push_front(self.name_buffer.clone());
            return NamedReferenceTokenizationResult::Success {
                reference: self.finish_matching_reference(last_match, error_callback, input),
            };
        }

        NamedReferenceTokenizationResult::Failed
    }

    pub(crate) fn step(
        &mut self,
        error_callback: EmitErrorFn,
        input: &BufferQueue,
    ) -> Result<Status, StrTendril> {
        loop {
            let Some(c) = tokenizer.peek(input) else {
                return Ok(Status::Stuck);
            };
            tokenizer.discard_char(input);

            match self.feed_character(c, tokenizer, input) {
                NamedReferenceTokenizationResult::Success { reference } => {
                    return Ok(Status::Done(reference));
                },
                NamedReferenceTokenizationResult::Failed => {
                    return Err(mem::take(&mut self.name_buffer));
                },
                NamedReferenceTokenizationResult::Continue => {},
            }
        }
    }

    pub(crate) fn notify_end_of_file(
        &mut self,
        error_callback: EmitErrorFn,
        input: &BufferQueue,
    ) -> Option<CharRef> {
        input.push_front(self.name_buffer.clone());
        if let Some(last_match) = self.last_match.take() {
            Some(self.finish_matching_reference(last_match, error_callback, input))
        } else {
            if self.name_buffer.ends_with(';') {
                println!("end of file and last is semicolon");
                emit_name_error(mem::take(&mut self.name_buffer), error_callback);
            }
            None
        }
    }

    /// Called whenever the tokenizer has finished matching a named reference.
    ///
    /// This method takes care of emitting appropriate errors and implement some legacy quirks.
    pub(crate) fn finish_matching_reference(
        &self,
        matched: Match,
        error_callback: EmitErrorFn,
        input: &BufferQueue,
    ) -> CharRef {
        let char_ref = resolve_unique_hash_value(matched.hash_value);
        let last_matched_codepoint = matched
            .matched_text
            .chars()
            .next_back()
            .expect("named character references cannot be empty");
        let first_codepoint_after_match = self.name_buffer.chars().next();

        // If the character reference was consumed as part of an attribute, and the last
        // character matched is not a U+003B SEMICOLON character (;), and the next input
        // character is either a U+003D EQUALS SIGN character (=) or an ASCII alphanumeric,
        // then, for historical reasons, flush code points consumed as a character
        // reference and switch to the return state.
        if self.is_in_attribute
            && last_matched_codepoint != ';'
            && first_codepoint_after_match.is_some_and(|c| c.is_ascii_alphanumeric() || c == '=')
        {
            input.push_front(matched.matched_text);
            return CharRef::EMPTY;
        }

        // If the last character matched is not a U+003B SEMICOLON character
        // (;), then this is a missing-semicolon-after-character-reference parse
        // error.
        if last_matched_codepoint != ';' {
            error_callback("Character reference does not end with semicolon");
        }
        char_ref
    }
}

// pub(crate) fn emit_name_error(name: StrTendril, tokenizer: &Tokenizer<Sink>) {
//     let msg = if tokenizer.opts.exact_errors {
//         Cow::from(format!("Invalid character reference &{}", name))
//     } else {
//         Cow::from("Invalid character reference")
//     };
//     tokenizer.emit_error(msg);
// }
