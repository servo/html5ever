// Copyright 2014-2025 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::tendril::StrTendril;

/// <https://html.spec.whatwg.org/multipage/#algorithm-for-extracting-a-character-encoding-from-a-meta-element>
pub(crate) fn extract_a_character_encoding_from_a_meta_element(
    input: StrTendril,
) -> Option<StrTendril> {
    // Step 1. Let position be a pointer into s, initially pointing at the start of the string.
    let mut position = 0;
    loop {
        // Step 2. Loop: Find the first seven characters in s after position that are an ASCII
        // case-insensitive match for the word "charset". If no such match is found, return nothing.
        loop {
            let candidate = input.as_bytes().get(position..position + "charset".len())?;
            if candidate.eq_ignore_ascii_case(b"charset") {
                break;
            }

            position += 1;
        }
        position += "charset".len();

        // Step 3. Skip any ASCII whitespace that immediately follow the word "charset" (there might not be any).
        position += input.as_bytes()[position..]
            .iter()
            .take_while(|byte| byte.is_ascii_whitespace())
            .count();

        // Step 4. If the next character is not a U+003D EQUALS SIGN (=), then move position to point just before
        // that next character, and jump back to the step labeled loop.
        if input.as_bytes()[position] == b'=' {
            break;
        }
    }
    // Skip the "="
    position += 1;

    // Step 5. Skip any ASCII whitespace that immediately follow the equals sign (there might not be any).
    position += input.as_bytes()[position..]
        .iter()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();

    // Step 6. Process the next character as follows:
    match input.as_bytes().get(position)? {
        quote @ (b'"' | b'\'') => {
            // Return the result of getting an encoding from the substring that is between this character
            // and the next earliest occurrence of this character.
            let length = input.as_bytes()[position + 1..]
                .iter()
                .position(|byte| byte == quote)?;
            Some(input.subtendril(position as u32 + 1, length as u32))
        },
        _ => {
            // Return the result of getting an encoding from the substring that consists of this character
            // up to but not including the first ASCII whitespace or U+003B SEMICOLON character (;),
            // or the end of s, whichever comes first.
            let length = input.as_bytes()[position..]
                .iter()
                .position(|byte| byte.is_ascii_whitespace() || *byte == b';');
            if let Some(length) = length {
                Some(input.subtendril(position as u32, length as u32))
            } else {
                Some(input.subtendril(position as u32, (input.len() - position) as u32))
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_element_without_charset() {
        assert_eq!(
            extract_a_character_encoding_from_a_meta_element(StrTendril::from_slice("foobar")),
            None
        );
    }

    #[test]
    fn meta_element_with_capitalized_charset() {
        assert_eq!(
            extract_a_character_encoding_from_a_meta_element(StrTendril::from_slice(
                "cHarSet=utf8"
            )),
            Some(StrTendril::from_slice("utf8"))
        );
    }

    #[test]
    fn meta_element_with_no_equals_after_charset() {
        assert_eq!(
            extract_a_character_encoding_from_a_meta_element(StrTendril::from_slice(
                "charset utf8"
            )),
            None
        );
    }

    #[test]
    fn meta_element_with_whitespace_around_equals() {
        assert_eq!(
            extract_a_character_encoding_from_a_meta_element(StrTendril::from_slice(
                "charset \t=\tutf8"
            )),
            Some(StrTendril::from_slice("utf8"))
        );
    }

    #[test]
    fn meta_element_with_quoted_value() {
        assert_eq!(
            extract_a_character_encoding_from_a_meta_element(StrTendril::from_slice(
                "charset='utf8'"
            )),
            Some(StrTendril::from_slice("utf8"))
        );
        assert_eq!(
            extract_a_character_encoding_from_a_meta_element(StrTendril::from_slice(
                "charset=\"utf8\""
            )),
            Some(StrTendril::from_slice("utf8"))
        );
        assert_eq!(
            extract_a_character_encoding_from_a_meta_element(StrTendril::from_slice(
                "charset='utf8"
            )),
            None
        );
        assert_eq!(
            extract_a_character_encoding_from_a_meta_element(StrTendril::from_slice(
                "charset=\"utf8"
            )),
            None
        );
    }

    #[test]
    fn meta_element_with_implicit_terminator() {
        assert_eq!(
            extract_a_character_encoding_from_a_meta_element(StrTendril::from_slice(
                "charset=utf8 foo"
            )),
            Some(StrTendril::from_slice("utf8"))
        );
        assert_eq!(
            extract_a_character_encoding_from_a_meta_element(StrTendril::from_slice(
                "charset=utf8;foo"
            )),
            Some(StrTendril::from_slice("utf8"))
        );
    }

    #[test]
    fn meta_element_with_content_type() {
        assert_eq!(
            extract_a_character_encoding_from_a_meta_element(StrTendril::from_slice(
                "text/html; charset=utf8"
            )),
            Some(StrTendril::from_slice("utf8"))
        );
    }
}
