/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

/// The spec replaces most characters in the ISO-2022 C1 control code range
/// (U+0080 through U+009F) with these characters, based on Windows 8-bit
/// codepages.
pub static c1_replacements: [Option<char>, ..32] = [
    Some('\u20ac'), None,           Some('\u201a'), Some('\u0192'),
    Some('\u201e'), Some('\u2026'), Some('\u2020'), Some('\u2021'),
    Some('\u02c6'), Some('\u2030'), Some('\u0160'), Some('\u2039'),
    Some('\u0152'), None,           Some('\u017d'), None,
    None,           Some('\u2018'), Some('\u2019'), Some('\u201c'),
    Some('\u201d'), Some('\u2022'), Some('\u2013'), Some('\u2014'),
    Some('\u02dc'), Some('\u2122'), Some('\u0161'), Some('\u203a'),
    Some('\u0153'), None,           Some('\u017e'), Some('\u0178'),
];
