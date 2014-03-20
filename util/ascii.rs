/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

/// If `c` is an ASCII letter, return the corresponding lowercase
/// letter, otherwise None.
pub fn lower_ascii_letter(c: char) -> Option<char> {
    c.to_ascii_opt()
        .filtered(|a| a.is_alpha())
        .map(|a| a.to_lower().to_char())
}

/// Map ASCII uppercase to lowercase; preserve other characters.
pub fn lower_ascii(c: char) -> char {
    lower_ascii_letter(c).unwrap_or(c)
}
