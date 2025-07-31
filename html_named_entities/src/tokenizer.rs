use crate::codegen::{resolve_unique_hash_value, Node, DAFSA_NODES};
use crate::{CharRef, InputSource};

use std::borrow::Cow;
use std::mem;

#[derive(Clone, Debug)]
pub(crate) struct Match {
    hash_value: usize,
    matched_text: String,
}

/// Tokenizer for named character references.
#[derive(Clone, Debug)]
pub struct NamedReferenceTokenizerState {
    current_node: &'static Node,
    /// Contains all characters that the tokenizer has consumed since the last match.
    ///
    /// We can't always know whether these characters will be part of a named reference until
    /// we consume more. For example, `&not` is a valid named character reference, but it might continue
    /// to form `&notin`. When we have consumed `&noti` then only the `i` will be in the name buffer
    /// since it is the only character that needs to be flushed when no further reference is found.
    name_buffer: String,
    hash_value: usize,

    /// The last match (last terminal node) that we found during the traversal.
    last_match: Option<Match>,

    is_in_attribute: bool,
}

/// The result of attempting to tokenize a named character reference.
pub enum NamedReferenceTokenizationResult {
    /// Tokenization is complete.
    Success(CharRef),
    /// The provided characters do not form a valid named reference and there is no
    /// valid continuation that would change that.
    ///
    /// Contains all the characters that have been tokenized so far.
    Failed(String),
    /// The tokenizer needs more input.
    Continue,
}

impl NamedReferenceTokenizerState {
    /// Construct a new tokenizer.
    ///
    /// `is_in_attribute` indicates whether the named reference that should be parsed
    /// is part of an attribute of a HTML tag.
    pub fn new(is_in_attribute: bool) -> Self {
        Self {
            current_node: &DAFSA_NODES[0],
            name_buffer: Default::default(),
            hash_value: Default::default(),
            last_match: None,
            is_in_attribute,
        }
    }

    /// Provide a single character to the tokenizer.
    pub fn feed_character<I, E>(
        &mut self,
        c: char,
        input: &I,
        error_callback: E,
    ) -> NamedReferenceTokenizationResult
    where
        I: InputSource,
        E: FnOnce(Cow<'static, str>),
    {
        self.name_buffer.push(c);
        if !c.is_ascii_alphanumeric() && c != ';' {
            return self.did_find_invalid_character(input, error_callback);
        }

        let code_point = c as u32 as u8;
        let mut next_node = None;
        for child in self.current_node.children() {
            if child.code_point() == code_point {
                next_node = Some(child);
                break;
            } else {
                self.hash_value += child.hash_value();
            }
        }

        let Some(next_node) = next_node else {
            return self.did_find_invalid_character(input, error_callback);
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

    fn did_find_invalid_character<I, E>(
        &mut self,
        input: &I,
        error_callback: E,
    ) -> NamedReferenceTokenizationResult
    where
        I: InputSource,
        E: FnOnce(Cow<'static, str>),
    {
        if let Some(last_match) = self.last_match.take() {
            input.push_front(self.name_buffer.clone());
            let reference = self.finish_matching_reference(last_match, input, error_callback);
            return NamedReferenceTokenizationResult::Success(reference);
        }

        NamedReferenceTokenizationResult::Failed(mem::take(&mut self.name_buffer))
    }

    /// Indicate to the tokenizer that all input has been consumed.
    pub fn notify_end_of_file<I, E>(&mut self, input: &I, error_callback: E) -> Option<CharRef>
    where
        I: InputSource,
        E: FnOnce(Cow<'static, str>),
    {
        input.push_front(self.name_buffer.clone());
        if let Some(last_match) = self.last_match.take() {
            Some(self.finish_matching_reference(last_match, input, error_callback))
        } else {
            if self.name_buffer.ends_with(';') {
                error_callback(Cow::from(format_name_error(&self.name_buffer)));
            }
            None
        }
    }

    /// Called whenever the tokenizer has finished matching a named reference.
    ///
    /// This method takes care of emitting appropriate errors and implement some legacy quirks.
    pub(crate) fn finish_matching_reference<I, E>(
        &self,
        matched: Match,
        input: &I,
        error_callback: E,
    ) -> CharRef
    where
        I: InputSource,
        E: FnOnce(Cow<'static, str>),
    {
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
            error_callback(Cow::from("Character reference does not end with semicolon"));
        }
        char_ref
    }
}

/// Format a error message for an invalid character reference.
pub fn format_name_error(matched_string: &str) -> String {
    format!("Invalid character reference: &{matched_string}")
}
