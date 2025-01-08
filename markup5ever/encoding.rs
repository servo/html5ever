// Copyright 2014-2025 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use encoding_rs::{DecoderResult, Encoding, UTF_16BE, UTF_8, WINDOWS_1252, X_USER_DEFINED};
use tendril::{fmt::Bytes, Tendril};

use crate::buffer_queue::BufferQueue;

/// <https://html.spec.whatwg.org/#concept-encoding-confidence>
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    Tentative,
    Certain,
    Irrelevant,
}

pub struct Decoder {
    inner: encoding_rs::Decoder,
    confidence: Confidence,
}

impl Decoder {
    pub fn new(encoding: &'static Encoding, confidence: Confidence) -> Self {
        Self {
            inner: encoding.new_decoder(),
            confidence,
        }
    }

    pub fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Returns `None` if the encoding should not be changed and `Some(encoding)` if the current encoding
    /// should be changed to `encoding`
    pub fn change_the_encoding_to(
        &mut self,
        mut new_encoding: &'static Encoding,
    ) -> Option<&'static Encoding> {
        let current_encoding = self.inner.encoding();
        // Step 1. If the encoding that is already being used to interpret the input stream is UTF-16BE/LE,
        // then set the confidence to certain and return. The new encoding is ignored; if it was anything
        // but the same encoding, then it would be clearly incorrect.
        if current_encoding == UTF_16BE || current_encoding == UTF_16BE {
            self.confidence = Confidence::Certain;
            return None;
        }

        // Step 2. If the new encoding is UTF-16BE/LE, then change it to UTF-8.
        if new_encoding == UTF_16BE || new_encoding == UTF_16BE {
            new_encoding = UTF_8;
        }

        // Step 3. If the new encoding is x-user-defined, then change it to windows-1252.
        if new_encoding == X_USER_DEFINED {
            new_encoding = WINDOWS_1252;
        }

        // Step 4. If the new encoding is identical or equivalent to the encoding that is already being used to interpret
        // the input stream, then set the confidence to certain and return. This happens when the encoding information found
        // in the file matches what the encoding sniffing algorithm determined to be the encoding, and in the second pass
        // through the parser if the first pass found that the encoding sniffing algorithm described in the earlier section
        // failed to find the right encoding.
        if current_encoding == new_encoding {
            self.confidence = Confidence::Certain;
            return None;
        }

        // Step 5. If all the bytes up to the last byte converted by the current decoder have the same
        // Unicode interpretations in both the current encoding and the new encoding, and if the user agent
        // supports changing the converter on the fly, then the user agent may change to the new converter
        // for the encoding on the fly. Set the document's character encoding and the encoding used to convert
        // the input stream to the new encoding, set the confidence to certain, and return.
        // NOTE: We don't support changing the converter on the fly

        // Step 6. Otherwise, restart the navigate algorithm, with historyHandling set to "replace" and
        // other inputs kept the same, but this time skip the encoding sniffing algorithm and instead just
        // set the encoding to the new encoding and the confidence to certain. Whenever possible, this should
        // be done without actually contacting the network layer (the bytes should be re-parsed from memory),
        // even if, e.g., the document is marked as not being cacheable. If this is not possible and contacting
        // the network layer would involve repeating a request that uses a method other than `GET`, then instead
        // set the confidence to certain and ignore the new encoding. The resource will be misinterpreted.
        // User agents may notify the user of the situation, to aid in application development.
        Some(new_encoding)
    }

    /// Decode the given chunk with the current encoding. The result will be pushed to the end
    /// of the input stream.
    pub fn decode(&mut self, chunk: &[u8], last: bool, output: &BufferQueue) {
        let mut remaining = chunk;
        loop {
            let mut out: Tendril<Bytes> = Tendril::new();
            let max_len = self
                .inner
                .max_utf8_buffer_length_without_replacement(remaining.len())
                .unwrap_or(8192)
                .min(8192);

            // SAFETY: encoding_rs::Decoder::decode_to_utf8_without_replacement is going to initialize
            // part of the buffer. We are only going to access the initialized segment.
            unsafe {
                out.push_uninitialized(max_len as u32);
            }

            let (result, bytes_read, bytes_written) = self
                .inner
                .decode_to_utf8_without_replacement(&remaining, &mut out, last);

            if bytes_written > 0 {
                let bytes_chunk = out.subtendril(0, bytes_written as u32);

                // SAFETY: encoding_rs::Decoder::decode_to_utf8_without_replacement writes valid utf8
                let utf8_chunk = unsafe { bytes_chunk.reinterpret_without_validating() };
                output.push_back(utf8_chunk);
            }

            if matches!(result, DecoderResult::Malformed(_, _)) {
                output.push_back("\u{FFFD}".into());
            }

            remaining = &remaining[bytes_read..];
            if remaining.is_empty() {
                return;
            }
        }
    }
}
