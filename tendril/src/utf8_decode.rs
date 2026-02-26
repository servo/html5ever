// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::fmt;
use crate::{Atomicity, Tendril};

use std::cmp;
use std::str;

/// The replacement character, U+FFFD. In lossy decoding, insert it for every decoding error.
pub(crate) const REPLACEMENT_CHARACTER: &str = "\u{FFFD}";

#[derive(Debug, Copy, Clone)]
pub(crate) enum DecodeError<'a> {
    /// In lossy decoding insert `valid_prefix`, then `"\u{FFFD}"`,
    /// then call `decode()` again with `remaining_input`.
    Invalid {
        valid_prefix: &'a str,
        invalid_sequence: &'a [u8],
    },

    /// Call the `incomplete_suffix.try_to_complete_codepoint` method with more input when available.
    /// If no more input is available, this is an invalid byte sequence.
    Incomplete {
        valid_prefix: &'a str,
        incomplete_suffix: IncompleteUtf8,
    },
}

#[derive(Debug, Copy, Clone)]
pub struct IncompleteUtf8 {
    pub buffer: [u8; 4],
    pub buffer_len: u8,
}

pub(crate) fn decode_utf8(input: &[u8]) -> Result<&str, DecodeError<'_>> {
    let error = match str::from_utf8(input) {
        Ok(valid) => return Ok(valid),
        Err(error) => error,
    };

    // FIXME: separate function from here to guide inlining?
    let (valid, after_valid) = input.split_at(error.valid_up_to());
    let valid = unsafe { str::from_utf8_unchecked(valid) };

    match error.error_len() {
        Some(invalid_sequence_length) => {
            let invalid = &after_valid[..invalid_sequence_length];
            Err(DecodeError::Invalid {
                valid_prefix: valid,
                invalid_sequence: invalid,
            })
        },
        None => Err(DecodeError::Incomplete {
            valid_prefix: valid,
            incomplete_suffix: IncompleteUtf8::new(after_valid),
        }),
    }
}

enum Utf8CompletionResult {
    NotEnoughInput,
    MalformedUtf8Buffer,
    Valid,
}

impl IncompleteUtf8 {
    fn new(bytes: &[u8]) -> Self {
        let mut buffer = [0, 0, 0, 0];
        let len = bytes.len();
        buffer[..len].copy_from_slice(bytes);

        Self {
            buffer,
            buffer_len: len as u8,
        }
    }

    fn take_buffer(&mut self) -> &[u8] {
        let len = self.buffer_len as usize;
        self.buffer_len = 0;
        &self.buffer[..len]
    }

    /// Consumes bytes from the input and attempts to form a valid utf8 codepoint.
    ///
    /// Returns how many bytes were consumed and whether a valid code point was found.
    fn try_complete_offsets(&mut self, input: &[u8]) -> (usize, Utf8CompletionResult) {
        let initial_buffer_len = self.buffer_len as usize;
        let copied_from_input;
        {
            let unwritten = &mut self.buffer[initial_buffer_len..];
            copied_from_input = cmp::min(unwritten.len(), input.len());
            unwritten[..copied_from_input].copy_from_slice(&input[..copied_from_input]);
        }
        let spliced = &self.buffer[..initial_buffer_len + copied_from_input];
        match str::from_utf8(spliced) {
            Ok(_) => {
                self.buffer_len = spliced.len() as u8;
                (copied_from_input, Utf8CompletionResult::Valid)
            },
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if valid_up_to > 0 {
                    let consumed = valid_up_to.checked_sub(initial_buffer_len).unwrap();
                    self.buffer_len = valid_up_to as u8;
                    (consumed, Utf8CompletionResult::Valid)
                } else {
                    match error.error_len() {
                        Some(invalid_sequence_length) => {
                            let consumed = invalid_sequence_length
                                .checked_sub(initial_buffer_len)
                                .unwrap();
                            self.buffer_len = invalid_sequence_length as u8;
                            (consumed, Utf8CompletionResult::MalformedUtf8Buffer)
                        },
                        None => {
                            self.buffer_len = spliced.len() as u8;
                            (copied_from_input, Utf8CompletionResult::NotEnoughInput)
                        },
                    }
                }
            },
        }
    }

    /// Attempts to complete the codepoint given the bytes from `input`.
    ///
    /// Returns `None` if more input is required to complete the codepoint. In this case, no
    /// input is consumed.
    ///
    /// Otherwise, returns either the decoded `&str` or malformed `&[u8]` and the remaining input.
    #[allow(clippy::type_complexity)]
    pub fn try_to_complete_codepoint<'input>(
        &mut self,
        input: &'input [u8],
    ) -> Option<(Result<&str, &[u8]>, &'input [u8])> {
        let (consumed, completion_result) = self.try_complete_offsets(input);
        let result = match completion_result {
            Utf8CompletionResult::NotEnoughInput => return None,
            Utf8CompletionResult::MalformedUtf8Buffer => Err(self.take_buffer()),
            Utf8CompletionResult::Valid => {
                Ok(unsafe { str::from_utf8_unchecked(self.take_buffer()) })
            },
        };
        let remaining_input = &input[consumed..];

        Some((result, remaining_input))
    }

    pub fn try_complete<A, F>(
        &mut self,
        mut input: Tendril<fmt::Bytes, A>,
        mut push_utf8: F,
    ) -> Result<Tendril<fmt::Bytes, A>, ()>
    where
        A: Atomicity,
        F: FnMut(Tendril<fmt::UTF8, A>),
    {
        let Some((result, remaining_input)) = self.try_to_complete_codepoint(&input) else {
            // Not enough input to complete codepoint
            return Err(());
        };

        push_utf8(Tendril::from_slice(result.unwrap_or(REPLACEMENT_CHARACTER)));
        let resume_at = input.len() - remaining_input.len();
        input.pop_front(resume_at as u32);
        Ok(input)
    }
}

impl<A> Tendril<fmt::Bytes, A>
where
    A: Atomicity,
{
    pub fn decode_utf8_lossy<F>(mut self, mut push_utf8: F) -> Option<IncompleteUtf8>
    where
        F: FnMut(Tendril<fmt::UTF8, A>),
    {
        loop {
            if self.is_empty() {
                return None;
            }
            let unborrowed_result = match decode_utf8(&self) {
                Ok(string) => {
                    debug_assert!(string.as_ptr() == self.as_ptr());
                    debug_assert!(string.len() == self.len());
                    Ok(())
                },
                Err(DecodeError::Invalid {
                    valid_prefix,
                    invalid_sequence,
                    ..
                }) => {
                    debug_assert!(valid_prefix.as_ptr() == self.as_ptr());
                    debug_assert!(valid_prefix.len() <= self.len());
                    Err((
                        valid_prefix.len(),
                        Err(valid_prefix.len() + invalid_sequence.len()),
                    ))
                },
                Err(DecodeError::Incomplete {
                    valid_prefix,
                    incomplete_suffix,
                }) => {
                    debug_assert!(valid_prefix.as_ptr() == self.as_ptr());
                    debug_assert!(valid_prefix.len() <= self.len());
                    Err((valid_prefix.len(), Ok(incomplete_suffix)))
                },
            };
            match unborrowed_result {
                Ok(()) => {
                    unsafe { push_utf8(self.reinterpret_without_validating()) }
                    return None;
                },
                Err((valid_len, and_then)) => {
                    if valid_len > 0 {
                        let subtendril = self.subtendril(0, valid_len as u32);
                        unsafe { push_utf8(subtendril.reinterpret_without_validating()) }
                    }
                    match and_then {
                        Ok(incomplete) => return Some(incomplete),
                        Err(offset) => {
                            push_utf8(Tendril::from_slice(REPLACEMENT_CHARACTER));
                            self.pop_front(offset as u32)
                        },
                    }
                },
            }
        }
    }
}
