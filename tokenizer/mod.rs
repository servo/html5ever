pub use self::tokens::{Doctype, Attributes, TagKind, StartTag, EndTag, Tag, Token};
pub use self::tokens::{DoctypeToken, TagToken, CommentToken, CharacterToken};

mod tokens;
mod states;

pub trait TokenSink {
    fn process_token(&mut self, token: Token);
}

fn letter_to_ascii_lowercase(c: char) -> Option<char> {
    c.to_ascii_opt()
        .filtered(|a| a.is_alpha())
        .map(|a| a.to_lower().to_char())
}


pub struct Tokenizer<'sink, Sink> {
    priv sink: &'sink mut Sink,
    priv state: states::State,

    // FIXME: The state machine guarantees the tag exists when
    // we need it, so we could eliminate the Option overhead.
    // Leaving it as Option for now, to find bugs.
    priv current_tag: Option<Tag>,
}

#[deriving(Eq)]
enum ConsumeCharResult {
    Reconsume,
    Finished,
}

impl<'sink, Sink: TokenSink> Tokenizer<'sink, Sink> {
    pub fn new(sink: &'sink mut Sink) -> Tokenizer<'sink, Sink> {
        Tokenizer {
            sink: sink,
            state: states::Data,
            current_tag: None,
        }
    }

    pub fn feed(&mut self, input: &str) {
        debug!("feeding {:s}", input);
        let mut it = input.chars(); //.peekable();
        loop {
            match self.state {
                // These states do something other than consume a single character.
                states::CharacterReferenceInData | states::CharacterReferenceInRcdata
                | states::CharacterReferenceInAttributeValue | states::BogusComment
                | states::MarkupDeclarationOpen | states::CdataSection => {
                    fail!("FIXME: state {:?} not implemented", self.state);
                }

                _ => {
                    match it.next() {
                        None => return,
                        Some(c) => {
                            while self.process_char(c) == Reconsume {
                                // reconsume
                            }
                        }
                    }
                }
            }
        }
    }

    fn emit(&mut self, token: Token) {
        self.sink.process_token(token);
    }

    fn create_tag(&mut self, kind: TagKind, c: char) {
        assert!(self.current_tag.is_none());
        let mut t = Tag::new(kind);
        t.name.push_char(c);
        self.current_tag = Some(t);
    }

    fn append_to_tag_name(&mut self, c: char) {
        self.current_tag.get_mut_ref().name.push_char(c);
    }

    // FIXME: explicitly represent the EOF character?
    // For now the plan is to handle EOF in a separate match.
    fn process_char(&mut self, c: char) -> ConsumeCharResult {
        let parse_error = || {
            error!("Parse error: saw {:?} in state {:?}", c, self.state);
        };

        debug!("Processing {:?} in state {:?}", c, self.state);
        match self.state {
            states::Data => match c {
                '&' => { self.state = states::CharacterReferenceInData; }
                '<' => { self.state = states::TagOpen; }
                '\0' => {
                    parse_error();
                    self.emit(CharacterToken('\0'));
                }
                _ => { self.emit(CharacterToken(c)); }
            },

            states::TagOpen => match c {
                '!' => { self.state = states::MarkupDeclarationOpen; }
                '/' => { self.state = states::EndTagOpen; }
                '?' => {
                    parse_error();
                    self.state = states::BogusComment;
                }
                _ => match letter_to_ascii_lowercase(c) {
                    Some(cl) => {
                        self.create_tag(StartTag, cl);
                        self.state = states::TagName;
                    }
                    None => {
                        parse_error();
                        self.emit(CharacterToken('<'));
                        self.state = states::Data;
                        return Reconsume;
                    }
                }
            },

            states::TagName => match c {
                '\t' | '\n' | '\x0C' | ' ' => { self.state = states::BeforeAttributeName; }
                '/' => { self.state = states::SelfClosingStartTag; }
                '>' => {
                    let tok = self.current_tag.take().unwrap();
                    self.emit(TagToken(tok));
                    self.state = states::Data;
                }
                '\0' => {
                    parse_error();
                    self.append_to_tag_name('\ufffd');
                }
                _ => match letter_to_ascii_lowercase(c) {
                    Some(cl) => { self.append_to_tag_name(cl); }
                    None     => { self.append_to_tag_name(c);  }
                }
            },

            states::EndTagOpen => match c {
                '>' => {
                    parse_error();
                    self.state = states::Data;
                }
                _ => match letter_to_ascii_lowercase(c) {
                    Some(cl) => {
                        self.create_tag(EndTag, cl);
                        self.state = states::TagName;
                    }
                    None => {
                        parse_error();
                        self.state = states::BogusComment;
                    }
                }
            },

            s => fail!("FIXME: state {:?} not implemented", s),
        }

        Finished

    }
}
