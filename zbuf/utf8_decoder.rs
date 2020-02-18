use std::error;
use std::fmt;
use std::io;
use std::mem;
use utf8::{self, DecodeError, Incomplete};
use BytesBuf;
use StrBuf;

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
/// This is the [`StrBuf::from_utf8_iter_lossy`](struct.StrBuf.html#method.from_utf8_iter_lossy)
/// method:
///
/// ```
/// # use zbuf::{BytesBuf, StrBuf, LossyUtf8Decoder};
/// pub fn from_utf8_iter_lossy<I>(iter: I) -> StrBuf
/// where I: IntoIterator, I::Item: Into<BytesBuf> {
///     let mut decoder = LossyUtf8Decoder::new();
///     let mut buf = StrBuf::new();
///     for item in iter {
///         buf.extend(decoder.feed(item.into()))
///     }
///     buf.extend(decoder.end());
///     buf
/// }
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
fn replacement_character(_: Utf8DecoderError) -> StrBuf {
    StrBuf::from(utf8::REPLACEMENT_CHARACTER)
}

impl Iterator for LossyUtf8Decoder {
    type Item = StrBuf;

    fn next(&mut self) -> Option<StrBuf> {
        self.0
            .next()
            .map(|result| result.unwrap_or_else(replacement_character))
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
/// This is the [`StrBuf::from_utf8_iter`](struct.StrBuf.html#method.from_utf8_iter) method:
///
/// ```
/// # use zbuf::{BytesBuf, StrBuf, StrictUtf8Decoder, Utf8DecoderError};
/// pub fn from_utf8_iter<I>(iter: I) -> Result<StrBuf, Utf8DecoderError>
/// where I: IntoIterator, I::Item: Into<BytesBuf> {
///     let mut decoder = StrictUtf8Decoder::new();
///     let mut buf = StrBuf::new();
///     for item in iter {
///         for result in decoder.feed(item.into()) {
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
    sum_chunks_len_so_far: usize,
}

impl StrictUtf8Decoder {
    /// Return a new decoder
    pub fn new() -> Self {
        StrictUtf8Decoder {
            incomplete_char: Incomplete::empty(),
            input_chunk: BytesBuf::new(),
            yield_error_next: false,
            sum_chunks_len_so_far: 0,
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
        assert!(
            self.exhausted(),
            "feeding Utf8Decoder before exhausting the previous input chunk"
        );
        self.sum_chunks_len_so_far += next_input_chunk.len();
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
    pub fn end(&mut self) -> Result<(), Utf8DecoderError> {
        assert!(
            self.exhausted(),
            "ending Utf8Decoder before exhausting the previous input chunk"
        );
        if self.incomplete_char.is_empty() {
            Ok(())
        } else {
            self.incomplete_char = Incomplete::empty();
            Err(Utf8DecoderError {
                position: self.sum_chunks_len_so_far,
            })
        }
    }

    fn take_input(&mut self) -> BytesBuf {
        mem::replace(&mut self.input_chunk, BytesBuf::new())
    }

    fn error(&self) -> Utf8DecoderError {
        Utf8DecoderError {
            position: self.sum_chunks_len_so_far - self.input_chunk.len(),
        }
    }

    #[cold]
    fn try_complete(&mut self) -> Option<Result<StrBuf, Utf8DecoderError>> {
        let input_chunk = &self.input_chunk;
        let completed = self.incomplete_char.try_complete(input_chunk);
        if let Some((result, remaining_input)) = completed {
            let consumed = input_chunk.len() - remaining_input.len();
            // `result` here is up to 4 bytes and therefore fits in an inline buffer,
            // so it is better to not try to share a heap allocation with `input_chunk`.
            self.input_chunk.pop_front(consumed);
            Some(match result {
                Ok(decoded) => Ok(StrBuf::from(decoded)),
                Err(_) => Err(self.error()),
            })
        } else {
            // Consumed the entire input
            self.input_chunk.clear();
            None
        }
    }
}

impl Iterator for StrictUtf8Decoder {
    type Item = Result<StrBuf, Utf8DecoderError>;

    fn next(&mut self) -> Option<Result<StrBuf, Utf8DecoderError>> {
        if self.yield_error_next {
            self.yield_error_next = false;
            return Some(Err(self.error()));
        }

        if self.input_chunk.is_empty() {
            return None;
        }

        if !self.incomplete_char.is_empty() {
            return self.try_complete();
        }

        let mut bytes;
        match utf8::decode(&self.input_chunk) {
            Ok(_) => bytes = self.take_input(),
            Err(DecodeError::Incomplete {
                valid_prefix,
                incomplete_suffix,
            }) => {
                self.incomplete_char = incomplete_suffix;
                let valid_prefix_len = valid_prefix.len();
                if valid_prefix_len == 0 {
                    self.input_chunk.clear();
                    return None;
                } else {
                    bytes = self.take_input();
                    bytes.truncate(valid_prefix_len)
                }
            }
            Err(DecodeError::Invalid {
                valid_prefix,
                invalid_sequence,
                remaining_input,
            }) => {
                if remaining_input.is_empty() {
                    let valid_prefix_len = valid_prefix.len();
                    if valid_prefix_len == 0 {
                        self.input_chunk.clear();
                        return Some(Err(self.error()));
                    } else {
                        self.yield_error_next = true;
                        bytes = self.take_input();
                        bytes.truncate(valid_prefix_len);
                    }
                } else {
                    let resume_at = valid_prefix.len() + invalid_sequence.len();
                    if valid_prefix.is_empty() {
                        self.input_chunk.pop_front(resume_at);
                        return Some(Err(self.error()));
                    } else {
                        self.yield_error_next = true;
                        bytes = self.input_chunk.clone();
                        bytes.truncate(valid_prefix.len());
                        self.input_chunk.pop_front(resume_at);
                    }
                }
            }
        }
        unsafe { Some(Ok(StrBuf::from_utf8_unchecked(bytes))) }
    }
}

/// The error type for [`StrictUtf8Decoder`](struct.StrictUtf8Decoder.html).
#[derive(Debug, Copy, Clone)]
pub struct Utf8DecoderError {
    position: usize,
}

impl Utf8DecoderError {
    /// Total number of bytes from the start of the stream to this invalid byte sequence.
    pub fn position(&self) -> usize {
        self.position
    }
}

impl fmt::Display for Utf8DecoderError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "invalid UTF-8 byte sequence at byte {}",
            self.position
        )
    }
}

impl error::Error for Utf8DecoderError {
    fn description(&self) -> &str {
        "invalid utf-8"
    }
}

impl From<Utf8DecoderError> for io::Error {
    fn from(error: Utf8DecoderError) -> Self {
        Self::new(io::ErrorKind::InvalidData, error)
    }
}
