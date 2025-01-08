use std::cell::RefCell;

use encoding_rs::Encoding;
use tendril::StrTendril;

use crate::buffer_queue::BufferQueue;
use crate::encoding::{Confidence, Decoder};

/// <https://html.spec.whatwg.org/#input-stream>
///
/// Internally the `InputStream` keeps track of the current
/// [insertion point](https://html.spec.whatwg.org/#insertion-point) by using
/// two seperate buffers.
pub struct InputStream {
    input: BufferQueue,
    decoder: RefCell<Decoder>,
}

impl InputStream {
    fn new(encoding: &'static Encoding) -> Self {
        Self {
            input: Default::default(),
            decoder: RefCell::new(Decoder::new(encoding, Confidence::Tentative)),
        }
    }

    pub fn append(&self, data: StrTendril) {
        self.input.push_back(data);
    }

    pub fn append_bytes(&self, data: &[u8]) {
        self.decoder
            .borrow_mut()
            .decode(data, false, &self.input);
    }

    pub fn code_points(&self) -> &BufferQueue {
        &self.input
    }

    /// Attempt to switch to another encoding.
    ///
    /// If the encoding was switched then the new encoding is returned. Note that the new encoding may be
    /// different from the one that this function was called with.
    pub fn maybe_switch_encoding(&self, encoding: &'static Encoding) -> Option<&'static Encoding> {
        if self.decoder.borrow().confidence() == Confidence::Tentative {
            if let Some(new_encoding) = self.decoder.borrow_mut().change_the_encoding_to(encoding) {
                return Some(new_encoding);
            }
        }
        None
    }

    /// Move any input that is left in the decoding stage to the end of the input stream
    pub fn finish_decoding_input(&self) {
        self.decoder
            .borrow_mut()
            .decode(&[], true, &self.input);
    }

    /// Remove all input from the stream
    pub fn clear(&self) {
        self.input.clear();
    }

    /// Swap the contents of the pending input queue with the provided queue
    pub fn swap_input_queue(&self, other: &BufferQueue) {
        self.input.swap(other);
    }
}

pub struct DecodingParser<Sink> {
    input_stream: InputStream,
    input_sink: Sink,
}

impl<Sink> DecodingParser<Sink>
where
    Sink: InputSink,
{
    pub fn new(sink: Sink, document_encoding: &'static Encoding) -> Self {
        Self {
            input_stream: InputStream::new(document_encoding),
            input_sink: sink,
        }
    }

    pub fn sink(&self) -> &Sink {
        &self.input_sink
    }

    pub fn input_stream(&self) -> &InputStream {
        &self.input_stream
    }

    /// Return an iterator that can be used to drive the parser
    pub fn parse(&self) -> impl Iterator<Item = ParserAction<Sink::Handle>> + '_ {
        self.input_sink
            .feed(self.input_stream.code_points())
            .filter_map(|sink_result| match sink_result {
                InputSinkResult::HandleScript(script) => Some(ParserAction::HandleScript(script)),
                InputSinkResult::MaybeStartOverWithEncoding(encoding) => self
                    .input_stream
                    .maybe_switch_encoding(encoding)
                    .map(ParserAction::StartOverWithEncoding),
            })
    }
}

pub enum ParserAction<Handle> {
    HandleScript(Handle),
    StartOverWithEncoding(&'static Encoding),
}

pub enum InputSinkResult<Handle> {
    HandleScript(Handle),
    MaybeStartOverWithEncoding(&'static Encoding),
}

pub trait InputSink {
    type Handle;

    fn feed(&self, input: &BufferQueue) -> impl Iterator<Item = InputSinkResult<Self::Handle>>;
}

impl<T> ParserAction<T> {
    pub fn map_script<U, F>(self, f: F) -> ParserAction<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::HandleScript(script) => ParserAction::HandleScript(f(script)),
            Self::StartOverWithEncoding(encoding) => ParserAction::StartOverWithEncoding(encoding),
        }
    }
}
