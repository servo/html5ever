#[macro_use] pub mod smallcharset;


/// Is the character an ASCII alphanumeric character?
pub fn is_ascii_alnum(c: char) -> bool {
    matches!(c, '0'...'9' | 'a'...'z' | 'A'...'Z')
}


#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use super::{is_ascii_alnum};

    test_eq!(is_alnum_a, is_ascii_alnum('a'), true);
    test_eq!(is_alnum_A, is_ascii_alnum('A'), true);
    test_eq!(is_alnum_1, is_ascii_alnum('1'), true);
    test_eq!(is_not_alnum_symbol, is_ascii_alnum('!'), false);
    test_eq!(is_not_alnum_nonascii, is_ascii_alnum('\u{a66e}'), false);
}
