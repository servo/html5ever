// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use mac::{matches, _tt_as_expr_hack};

/// Is the character an ASCII alphanumeric character?
pub fn is_ascii_alnum(c: char) -> bool {
    matches!(c, '0'..='9' | 'a'..='z' | 'A'..='Z')
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use super::is_ascii_alnum;
    use mac::test_eq;

    test_eq!(is_alnum_a, is_ascii_alnum('a'), true);
    test_eq!(is_alnum_A, is_ascii_alnum('A'), true);
    test_eq!(is_alnum_1, is_ascii_alnum('1'), true);
    test_eq!(is_not_alnum_symbol, is_ascii_alnum('!'), false);
    test_eq!(is_not_alnum_nonascii, is_ascii_alnum('\u{a66e}'), false);
}
