use std::cell::RefCell;

use encoding_rs::Encoding;
use tendril::StrTendril;

use crate::buffer_queue::BufferQueue;
use crate::encoding::{Confidence, Decoder};

/// <https://html.spec.whatwg.org/#input-stream>
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
        self.decoder.borrow_mut().decode(data, false, &self.input);
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
        self.decoder.borrow_mut().decode(&[], true, &self.input);
    }

    /// Remove all input from the stream
    pub fn clear(&self) {
        self.input.clear();
    }
}

pub struct DecodingParser<Sink> {
    /// Data received from `document.write`
    script_input: BufferQueue,
    input_stream: InputStream,
    input_sink: Sink,
}

impl<Sink> DecodingParser<Sink>
where
    Sink: InputSink,
{
    pub fn new(sink: Sink, document_encoding: &'static Encoding) -> Self {
        Self {
            script_input: Default::default(),
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

    /// Returns an iterator that can be used to drive the parser
    pub fn document_write<'a>(
        &'a self,
        input: &'a BufferQueue,
    ) -> impl Iterator<Item = ParserAction<Sink::Handle>> + use<'a, Sink> {
        debug_assert!(
            self.script_input.is_empty(),
            "Should not parse input from document.write while the parser is suspended"
        );

        self.input_sink
            .feed(&input)
            .filter_map(move |sink_result| match sink_result {
                InputSinkResult::HandleScript(script) => Some(ParserAction::HandleScript(script)),
                InputSinkResult::MaybeStartOverWithEncoding(encoding) => self
                    .input_stream
                    .maybe_switch_encoding(encoding)
                    .map(ParserAction::StartOverWithEncoding),
            })
    }

    /// End a `document.write` transaction, appending any input that was not yet parsed to the
    /// current insertion point, behind any input that was received reentrantly during this transaction.
    pub fn push_script_input(&self, input: &BufferQueue) {
        while let Some(chunk) = input.pop_front() {
            self.script_input.push_back(chunk);
        }
    }

    /// Notifies the parser that it has been unblocked and parsing can resume
    pub fn notify_parser_blocking_script_loaded(&self) {
        // Move pending script input to the front of the input stream
        self.script_input.swap_with(&self.input_stream.input);
        while let Some(chunk) = self.script_input.pop_front() {
            self.input_stream.input.push_back(chunk);
        }
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

    fn feed<'a>(
        &'a self,
        input: &'a BufferQueue,
    ) -> impl Iterator<Item = InputSinkResult<Self::Handle>> + 'a;
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
