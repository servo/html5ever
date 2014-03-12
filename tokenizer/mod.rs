pub use self::tokens::{Doctype, Attributes, TagKind, StartTag, EndTag, Tag, Token};
pub use self::tokens::{DoctypeToken, TagToken, CommentToken, CharacterToken};

mod tokens;
mod states;

pub trait TokenSink {
    fn process_token(&mut self, token: Token);
}

fn ascii_letter(c: char) -> Option<char> {
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
}

// A little DSL for common state machine behaviors.
macro_rules! go (
    ( to $state:ident $($rest:tt)* ) => ({
        self.state = states::$state;
        go!($($rest)*);
    });

    ( emit $c:expr $($rest:tt)* ) => ({
        self.emit(CharacterToken($c));
        go!($($rest)*);
    });

    ( error $($rest:tt)* ) => ({
        error!("Parse error: saw {:?} in state {:?}", c, self.state)
        go!($($rest)*);
    });

    ( create_tag $kind:expr $c:expr $($rest:tt)* ) => ({
        self.create_tag($kind, $c);
        go!($($rest)*);
    });

    ( reconsume ) => ({
        return Reconsume;
    });

    () => ({});
)

impl<'sink, Sink: TokenSink> Tokenizer<'sink, Sink> {
    // FIXME: explicitly represent the EOF character?
    // For now the plan is to handle EOF in a separate match.
    fn process_char(&mut self, c: char) -> ConsumeCharResult {
        debug!("Processing {:?} in state {:?}", c, self.state);
        match self.state {
            states::Data => match c {
                '&'  => go!(to CharacterReferenceInData),
                '<'  => go!(to TagOpen),
                '\0' => go!(error emit '\0'),
                _    => go!(emit c),
            },

            states::TagOpen => match c {
                '!' => go!(to MarkupDeclarationOpen),
                '/' => go!(to EndTagOpen),
                '?' => go!(error to BogusComment),
                _ => match ascii_letter(c) {
                    Some(cl) => go!(create_tag StartTag cl to TagName),
                    None     => go!(error emit '<' to Data reconsume)
                }
            },

            states::TagName => match c {
                '\t' | '\n' | '\x0C' | ' ' => go!(to BeforeAttributeName),
                '/' => go!(to SelfClosingStartTag),
                '>' => {
                    let tok = self.current_tag.take().unwrap();
                    self.emit(TagToken(tok));
                    go!(to Data);
                }
                '\0' => {
                    go!(error);
                    self.append_to_tag_name('\ufffd');
                }
                _ => self.append_to_tag_name(
                    ascii_letter(c).unwrap_or(c))
            },

            states::EndTagOpen => match c {
                '>' => go!(error to Data),
                _ => match ascii_letter(c) {
                    Some(cl) => {
                        self.create_tag(EndTag, cl);
                        go!(to TagName);
                    }
                    None => go!(error to BogusComment)
                }
            },

            s => fail!("FIXME: state {:?} not implemented", s),
        }

        Finished

    }
}
