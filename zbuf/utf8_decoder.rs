use BytesBuf;
use StrBuf;
use std::mem;
use utf8::{self, Incomplete, DecodeError};

/// A “zero-copy” incremental lossy UTF-8 decoder.
///
/// * **“Zero-copy”**:
///   String buffers produced by the decoder are either inline
///   or share a heap allocation with an input bytes buffer.
///   The decoder never allocates memory.
///
/// * **Incremental**:
///   The input doesn’t need to be provided all at once in a contiguous buffer.
///   Whatever input is available can be decoded while waiting for more to arrive,
///   for example from the network.
///   The decoder takes care of reconstructing `char` code points correctly
///   if their UTF-8 bytes span multiple input chunks.
///
///   If the entire input *is* available all at once, consider using
///   [`StrBuf::from_utf8_lossy`](struct.StrBuf.html#method.from_utf8_lossy) instead.
///
/// * **Lossy**:
///   Invalid byte sequences (decoding errors) are replaced with the replacement character U+FFFD.
///
/// # Examples
///
/// ```
/// # use zbuf::{BytesBuf, LossyUtf8Decoder};
/// let chunks = [
///     &[0xF0, 0x9F][..],
///     &[0x8E],
///     &[0x89, 0xF0, 0x9F],
/// ];
/// let mut decoder = LossyUtf8Decoder::new();
/// let mut bufs = Vec::new();
/// for chunk in &chunks {
///     bufs.extend(decoder.feed(BytesBuf::from(chunk)))
/// }
/// bufs.extend(decoder.end());
/// let slices = bufs.iter().map(|b| &**b).collect::<Vec<&str>>();
/// assert_eq!(slices, ["🎉", "�"]);
/// ```
pub struct LossyUtf8Decoder(StrictUtf8Decoder);

impl LossyUtf8Decoder {
    /// Return a new decoder
    pub fn new() -> Self {
        LossyUtf8Decoder(StrictUtf8Decoder::new())
    }

    /// Provide more bytes input to decode. Returns an iterator of `StrBuf`.
    ///
    /// The returned iterator must be exhausted (consumed until `.next()` returns `None`)
    /// before the next call to `.feed(…)` or `.end()`.
    ///
    /// # Panics
    ///
    /// Panics if the input of a previous `.feed(…)` call was not consumed entirely.
    pub fn feed(&mut self, next_input_chunk: BytesBuf) -> &mut Self {
        self.0.feed(next_input_chunk);
        self
    }

    /// Signal the end of the input. This may return one replacement character U+FFFD.
    ///
    /// Failing to call this method may result in incorrect decoding.
    ///
    /// Note that `Option<T>` implements `IntoIterator`,
    /// so it can be given for example to an `extend` method.
    ///
    /// # Panics
    ///
    /// Panics if the input of a previous `.feed(…)` call was not consumed entirely.
    pub fn end(&mut self) -> Option<StrBuf> {
        self.0.end().err().map(replacement_character)
    }
}

// FIXME: Make this a `const` item when const_fn is stable
#[inline]
fn replacement_character(_: ()) -> StrBuf {
    StrBuf::from(utf8::REPLACEMENT_CHARACTER)
}

impl Iterator for LossyUtf8Decoder {
    type Item = StrBuf;

    fn next(&mut self) -> Option<StrBuf> {
        self.0.next().map(|result| result.unwrap_or_else(replacement_character))
    }
}

/// A “zero-copy” incremental strict UTF-8 decoder.
///
/// * **“Zero-copy”**:
///   String buffers produced by the decoder are either inline
///   or share a heap allocation with an input bytes buffer.
///   The decoder never allocates memory.
///
/// * **Incremental**:
///   The input doesn’t need to be provided all at once in a contiguous buffer.
///   Whatever input is available can be decoded while waiting for more to arrive,
///   for example from the network.
///   The decoder takes care of reconstructing `char` code points correctly
///   if their UTF-8 bytes span multiple input chunks.
///
///   If the entire input *is* available all at once, consider using
///   [`StrBuf::from_utf8_lossy`](struct.StrBuf.html#method.from_utf8_lossy) instead.
///
/// * **Strict**:
///   Invalid byte sequences are represented as `Result::Err`
///
/// # Examples
///
/// ```
/// # use zbuf::{BytesBuf, StrBuf, StrictUtf8Decoder};
/// pub fn from_utf8_iter<I>(iter: I) -> Result<StrBuf, ()>
/// where I: IntoIterator<Item=BytesBuf> {
///     let mut decoder = StrictUtf8Decoder::new();
///     let mut buf = StrBuf::new();
///     for item in iter {
///         for result in decoder.feed(item) {
///             buf.push_buf(&result?)
///         }
///     }
///     decoder.end()?;
///     Ok(buf)
/// }
/// ```
pub struct StrictUtf8Decoder {
    input_chunk: BytesBuf,
    incomplete_char: Incomplete,
    yield_error_next: bool,
}

impl StrictUtf8Decoder {
    /// Return a new decoder
    pub fn new() -> Self {
        StrictUtf8Decoder {
            incomplete_char: Incomplete::empty(),
            input_chunk: BytesBuf::new(),
            yield_error_next: false,
        }
    }

    fn exhausted(&self) -> bool {
        self.input_chunk.is_empty() && !self.yield_error_next
    }

    /// Provide more bytes input to decode. Returns an iterator of `Result<StrBuf, ()>`.
    ///
    /// The returned iterator must be exhausted (consumed until `.next()` returns `None`)
    /// before the next call to `.feed(…)` or `.end()`.
    ///
    /// # Panics
    ///
    /// Panics if the input of a previous `.feed(…)` call was not consumed entirely.
    pub fn feed(&mut self, next_input_chunk: BytesBuf) -> &mut Self {
        assert!(self.exhausted(), "feeding Utf8Decoder before exhausting the previous input chunk");
        self.input_chunk = next_input_chunk;
        self
    }

    /// Signal the end of the input. This may return an error.
    ///
    /// Failing to call this method may result in incorrect decoding.
    ///
    /// # Panics
    ///
    /// Panics if the input of a previous `.feed(…)` call was not consumed entirely.
    pub fn end(&mut self) -> Result<(), ()> {
        assert!(self.exhausted(), "ending Utf8Decoder before exhausting the previous input chunk");
        if self.incomplete_char.is_empty() {
            Ok(())
        } else {
            self.incomplete_char = Incomplete::empty();
            Err(())
        }
    }

    fn take_input(&mut self) -> BytesBuf {
        mem::replace(&mut self.input_chunk, BytesBuf::new())
    }

    #[cold]
    fn try_complete(&mut self) -> Option<Result<StrBuf, ()>> {
        // FIXME: simplify when borrows are non-lexical
        let unborrowed = {
            let input_chunk = &self.input_chunk;
            self.incomplete_char.try_complete(input_chunk)
                .map(|(result, remaining_input)| {
                    let consumed = input_chunk.len() - remaining_input.len();
                    // `result` here is up to 4 bytes and therefore fits in an inline buffer,
                    // so it is better to not try to share a heap allocation with `input_chunk`.
                    let result = result.map(StrBuf::from).map_err(|_| ());
                    (consumed, result)
                })
        };
        match unborrowed {
            None => {
                // Consumed the entire input
                self.input_chunk = BytesBuf::new();
                None
            }
            Some((consumed_prefix_len, decoded)) => {
                self.input_chunk.pop_front(consumed_prefix_len);
                Some(decoded)
            }
        }
    }
}

impl Iterator for StrictUtf8Decoder {
    type Item = Result<StrBuf, ()>;

    fn next(&mut self) -> Option<Result<StrBuf, ()>> {
        if self.yield_error_next {
            self.yield_error_next = false;
            return Some(Err(()))
        }

        if self.input_chunk.is_empty() {
            return None
        }

        if !self.incomplete_char.is_empty() {
            return self.try_complete()
        }

        struct IsIncomplete;

        // FIXME: simplify when borrows are non-lexical
        let unborrowed = match utf8::decode(&self.input_chunk) {
            Ok(_) => Ok(()),
            Err(DecodeError::Incomplete { valid_prefix, incomplete_suffix }) => {
                self.incomplete_char = incomplete_suffix;
                Err((valid_prefix.len(), Ok(IsIncomplete)))
            }
            Err(DecodeError::Invalid { valid_prefix, invalid_sequence, remaining_input  }) => {
                let resume_at = if remaining_input.is_empty() {
                    None
                } else {
                    Some(valid_prefix.len() + invalid_sequence.len())
                };
                Err((valid_prefix.len(), Err(resume_at)))
            }
        };

        let mut bytes;
        match unborrowed {
            Ok(()) => {
                bytes = self.take_input()
            }

            Err((0, Ok(IsIncomplete))) => {
                self.input_chunk = BytesBuf::new();
                return None
            }
            Err((valid_prefix_len, Ok(IsIncomplete))) => {
                bytes = self.take_input();
                bytes.truncate(valid_prefix_len)
            }

            Err((0, Err(None))) => {
                self.input_chunk = BytesBuf::new();
                return Some(Err(()))
            }
            Err((0, Err(Some(resume_at)))) => {
                self.input_chunk.pop_front(resume_at);
                return Some(Err(()))
            }
            Err((valid_prefix_len, Err(None))) => {
                self.yield_error_next = true;
                bytes = self.take_input();
                bytes.truncate(valid_prefix_len);
            }
            Err((valid_prefix_len, Err(Some(resume_at)))) => {
                self.yield_error_next = true;
                bytes = self.input_chunk.clone();
                bytes.truncate(valid_prefix_len);
                self.input_chunk.pop_front(resume_at);
            }
        }
        unsafe {
            Some(Ok(StrBuf::from_utf8_unchecked(bytes)))
        }
    }
}
