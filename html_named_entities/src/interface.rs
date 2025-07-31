/// A source of characters for the tokenizer.
pub trait InputSource {
    /// Inserts the given value at the beginning of the input stream, such that
    /// it will be consumed next.
    fn push_front(&self, value: String);
}

/// A parsed character reference
#[derive(Clone, Copy, Debug)]
pub struct CharRef {
    /// The resulting character(s)
    pub chars: [char; 2],

    /// How many slots in `chars` are valid?
    pub num_chars: u8,
}

impl CharRef {
    /// A character reference that contains no characters.
    pub const EMPTY: CharRef = CharRef {
        chars: ['\0', '\0'],
        num_chars: 0,
    };
}
