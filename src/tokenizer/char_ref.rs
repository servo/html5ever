pub use self::Status::*;
pub use self::XRef::*;

use std::char::from_u32;
use std::borrow::Cow::Borrowed;

use super::{XTokenSink, XmlTokenizer};
use self::XState::*;
use tendril::StrTendril;


pub enum Status {
    Stuck,
    Progress,
    Done,
}

#[derive(Debug)]
enum XState {
    XBegin,
    XNumeric(u32),
    XNumericSemicolon,
    XReference,
    XOctothorpe,
}

pub enum XRef {
    NamedXRef(StrTendril),
    CharXData(StrTendril),
    NoReturn,
}

pub struct XCharRefTokenizer {
    state: XState,
    result: Option<XRef>,
    hex_marker: Option<char>,

    num: u32,
    num_too_big: bool,

    seen_digit: bool,
    name_buf_opt: Option<StrTendril>,
}
impl XCharRefTokenizer {

    // NB: We assume that we have an additional allowed character iff we're
    // tokenizing in an attribute value.
    pub fn new() -> XCharRefTokenizer {
        XCharRefTokenizer {
            state: XState::XBegin,
            result: None,
            hex_marker: None,
            num: 0,
            num_too_big: false,
            seen_digit: false,
            name_buf_opt: None,
        }
    }

    // A CharRefTokenizer can only tokenize one character reference,
    // so this method consumes the tokenizer.
    pub fn get_result(self) -> XRef {
        self.result.expect("get_result called before done")
    }

    fn name_buf_mut<'t>(&'t mut self) -> &'t mut StrTendril {
        self.name_buf_opt.as_mut()
            .expect("name_buf missing in named character reference")
    }

    fn name_buf<'t>(&'t self) -> &'t StrTendril {
        self.name_buf_opt.as_ref()
            .expect("name_buf missing in named character reference")
    }
}



impl XCharRefTokenizer {

    pub fn step<Sink: XTokenSink>(
        &mut self,
        tokenizer: &mut XmlTokenizer<Sink>
        ) -> Status {

        if self.result.is_some() {
            return Done;
        }

        debug!("Xml char ref tokenizer stepping in state {:?}", self.state);
        match self.state {
            XBegin => self.do_begin(tokenizer),
            XNumeric(base) => self.do_numeric(tokenizer, base),
            XNumericSemicolon => self.do_numeric_semicolon(tokenizer),
            XReference => self.do_reference(tokenizer),
            XOctothorpe => self.do_octothorpe(tokenizer),
        }
    }

    fn do_begin<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        match unwrap_or_return!(tokenizer.peek(), Stuck) {
            '\t' | '\n' | '\x0C' | ' ' | '<' | '&' | '%'
                => self.finish_none(),

            '#' => {
                tokenizer.discard_char();
                self.state = XState::XOctothorpe;
                Progress
            }

            _ => {
                self.state = XState::XReference;
                self.name_buf_opt = Some(StrTendril::new());
                Progress
            }
        }
    }

    fn do_octothorpe<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        let c = unwrap_or_return!(tokenizer.peek(), Stuck);
        match c {
            'x' | 'X' => {
                tokenizer.discard_char();
                self.hex_marker = Some(c);
                self.state = XNumeric(16);
            }

            _ => {
                self.hex_marker = None;
                self.state = XNumeric(10);
            }
        }
        Progress
    }


    fn do_numeric<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>, base: u32) -> Status {
        let c = unwrap_or_return!(tokenizer.peek(), Stuck);
        match c.to_digit(base as u32) {
            Some(n) => {
                tokenizer.discard_char();
                self.num *= base;
                if self.num > 0x10FFFF {
                    // We might overflow, and the character is definitely invalid.
                    // We still parse digits and semicolon, but don't use the result.
                    self.num_too_big = true;
                }
                self.num += n as u32;
                self.seen_digit = true;
                Progress
            }

            None if !self.seen_digit => self.unconsume_numeric(tokenizer),

            None => {
                self.state = XNumericSemicolon;
                Progress
            }
        }
    }

    fn do_numeric_semicolon<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        match unwrap_or_return!(tokenizer.peek(), Stuck) {
            ';' => tokenizer.discard_char(),
            _   => tokenizer.emit_error(Borrowed("Semicolon missing after numeric character reference")),
        };
        self.finish_numeric(tokenizer)
    }

    fn do_reference<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        let c = unwrap_or_return!(tokenizer.get_char(), Stuck);
        if is_xml_namechar(&c) {
            self.name_buf_mut().push_char(c);
            Progress
        } else if  c == ';' {
            self.finish_reference(tokenizer)
        } else {
            tokenizer.unconsume(StrTendril::from_char(c));
            let temp = self.name_buf().clone();
            self.finish_text(temp)
        }

    }

    pub fn end_of_file<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) {


        while self.result.is_none() {
            match self.state {
                XBegin => { self.finish_none(); },

                XNumeric(_) if !self.seen_digit
                    => { self.unconsume_numeric(tokenizer); },

                XNumeric(_) | XState::XNumericSemicolon => {
                    tokenizer.emit_error(Borrowed("EOF in numeric character reference"));
                    self.finish_numeric(tokenizer);
                },

                XReference => {
                    tokenizer.emit_error(Borrowed("EOF in reference"));
                    self.finish_reference(tokenizer);
                },

                XOctothorpe => {
                    tokenizer.emit_error(Borrowed("EOF after '#' in character reference"));
                    self.finish_text(StrTendril::from("#"));
                },
            }
        }
    }

    fn finish_none(&mut self) -> Status {
        self.result = Some(NoReturn);
        Done
    }

    fn finish_text(&mut self, text: StrTendril) -> Status {
        self.result = Some(CharXData(text));
        Done
    }

    fn finish_reference<Sink: XTokenSink>(&mut self,
        tokenizer: &mut XmlTokenizer<Sink>) -> Status {

        use std::mem::replace;

        match self.name_buf_opt {
            Some(ref mut c) if c.len() > 0 => {
                self.result = Some(NamedXRef(replace(c, StrTendril::new())));
            },
            _ => {
                tokenizer.emit_error(Borrowed("empty reference"));
                self.result = Some(NoReturn);
            }
        };
        Done
    }

    fn finish_numeric<Sink: XTokenSink>(&mut self, tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        fn conv(n: u32) -> char {
            from_u32(n).expect("invalid char missed by error handling cases")
        }

        let (c, error) = match self.num {
            n if (n > 0x10FFFF) || self.num_too_big => ('\u{fffd}', true),
            n => (conv(n), false),
        };

        if error {
            let msg = format_if!(tokenizer.opts.exact_errors,
                "Invalid numeric character reference",
                "Invalid numeric character reference value 0x{:06X}", self.num);
            tokenizer.emit_error(msg);
        }
        self.result = Some(CharXData(StrTendril::from_char(c)));
        Done
    }

    fn unconsume_numeric<Sink: XTokenSink>(&mut self, tokenizer: &mut XmlTokenizer<Sink>) -> Status {
        let mut unconsume = StrTendril::from("#");
        match self.hex_marker {
            Some(c) => unconsume.push_char(c),
            None => (),
        }


        tokenizer.emit_error(Borrowed("Numeric character reference without digits"));
        self.finish_text(unconsume)
    }
}

/// Determines if the character is a valid name character
/// according to XML 1.1 spec
fn is_xml_namechar(c: &char) -> bool {
    match *c {
        'A'...'Z' | 'a'...'z' |  '0'...'9'
        | ':' | '_' | '-' | '.' | '\u{B7}' | '\u{C0}'...'\u{D6}'
        | '\u{D8}'...'\u{F6}' | '\u{370}'...'\u{37D}'
        | '\u{37F}'...'\u{1FFF}' | '\u{200C}'...'\u{200D}'
        | '\u{0300}'...'\u{036F}' | '\u{203F}'...'\u{2040}'
        | '\u{2070}'...'\u{218F}' | '\u{2C00}'...'\u{2FEF}'
        | '\u{3001}'...'\u{D7FF}' | '\u{F900}'...'\u{FDCF}'
        | '\u{FDF0}'...'\u{FFFD}' | '\u{10000}'...'\u{EFFFF}'
        => true,
        _ => false,
    }
}
