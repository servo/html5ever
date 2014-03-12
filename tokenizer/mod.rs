pub use self::tokens::{Doctype, Attributes, TagKind, StartTag, EndTag, Tag, Token};
pub use self::tokens::{DoctypeToken, TagToken, CommentToken, CharacterToken};

use self::states::{RawLessThanSign, RawEndTagOpen, RawEndTagName};
use self::states::{Rcdata, ScriptData, ScriptDataEscaped};

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

    /// Last start tag name, for use in checking "appropriate end tag".
    priv last_start_tag_name: Option<~str>,

    /// The "temporary buffer" mentioned in the spec.
    priv temp_buf: ~str
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
            last_start_tag_name: None,
            temp_buf: ~"",
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

                _ => match it.next() {
                    None => return,
                    Some(c) => {
                        while self.process_char(c) == Reconsume {
                            // FIXME: this is not correct when state changes to one
                            // of the above!
                        }
                    }
                }
            }
        }
    }

    fn parse_error(&self, c: char) {
        error!("Parse error: saw {:?} in state {:?}", c, self.state);
    }

    fn emit_char(&self, c: char) {
        self.sink.process_token(CharacterToken(c));
    }

    fn emit_current_tag(&mut self) {
        let tag = self.current_tag.take().unwrap();
        match tag.kind {
            StartTag => self.last_start_tag_name = Some(tag.name.clone()),
            _ => ()
        }
        self.sink.process_token(TagToken(tag));
    }

    fn emit_temp_buf(&mut self) {
        // FIXME: Add a multi-character token and move temp_buf into it, something like
        //     self.sink.process_token(StringToken(
        //         replace(&mut self.temp_buf, ~"")))
        //
        // Need to make sure that clearing on emit is spec-compatible.
        //
        // Until then we reuse the same buffer allocation forever.
        for c in self.temp_buf.chars() {
            self.emit_char(c);
        }
    }

    fn clear_temp_buf(&mut self) {
        // Do this without a new allocation.
        self.temp_buf.truncate(0);
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

    fn have_appropriate_end_tag(&self) -> bool {
        match (self.last_start_tag_name.as_ref(), self.current_tag.as_ref()) {
            (Some(last), Some(tag)) =>
                (tag.kind == EndTag) && (tag.name.as_slice() == last.as_slice()),
            _ => false
        }
    }
}

// Shorthand for common state machine behaviors.
macro_rules! shorthand (
    ( to          $state:ident             ) => ( self.state = states::$state;        );
    ( to_raw      $state:ident $kind:ident ) => ( self.state = states::$state($kind); );
    ( emit        $c:expr                  ) => ( self.emit_char($c);                 );
    ( create_tag  $kind:expr   $c:expr     ) => ( self.create_tag($kind, $c);         );
    ( append_tag  $c:expr                  ) => ( self.append_to_tag_name($c);        );
    ( emit_tag                             ) => ( self.emit_current_tag();            );
    ( append_temp $c:expr                  ) => ( self.temp_buf.push_char($c);        );
    ( emit_temp                            ) => ( self.emit_temp_buf();               );
    ( clear_temp                           ) => ( self.clear_temp_buf();              );

    // NB: Deliberate capture of 'c'
    ( error                                ) => ( self.parse_error(c);                );
)

// A little DSL for sequencing shorthand actions.
macro_rules! go (
    // A pattern like $($cmd:tt)* ; $($rest:tt)* causes parse ambiguity.
    // We have to tell the parser how much lookahead we need.
    ( $a:tt             ; $($rest:tt)* ) => ({ shorthand!($a);       go!($($rest)*); });
    ( $a:tt $b:tt       ; $($rest:tt)* ) => ({ shorthand!($a $b);    go!($($rest)*); });
    ( $a:tt $b:tt $c:tt ; $($rest:tt)* ) => ({ shorthand!($a $b $c); go!($($rest)*); });

    // These can only come at the end.
    // FIXME: Come up with a better name for 'finish'.
    ( reconsume ) => ( return Reconsume; );
    ( finish    ) => ( return Finished;  );

    // If nothing else matched, it's a single command
    ( $($cmd:tt)+ ) => ( shorthand!($($cmd)+); );

    // or nothing.
    () => (());
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
                '\0' => go!(error; emit '\0'),
                _    => go!(emit c),
            },

            // RCDATA, RAWTEXT, script, or script escaped
            states::RawData(kind) => match c {
                '&' if kind == Rcdata            => go!(to CharacterReferenceInRcdata),
                '-' if kind == ScriptDataEscaped => go!(to ScriptDataEscapedDash; emit '-'),
                '<'  => go!(to_raw RawLessThanSign kind),
                '\0' => go!(error; emit '\ufffd'),
                _    => go!(emit c),
            },

            states::Plaintext => match c {
                '\0' => go!(error; emit '\ufffd'),
                _    => go!(emit c),
            },

            states::TagOpen => match c {
                '!' => go!(to MarkupDeclarationOpen),
                '/' => go!(to EndTagOpen),
                '?' => go!(error; to BogusComment),
                _ => match ascii_letter(c) {
                    Some(cl) => go!(create_tag StartTag cl; to TagName),
                    None     => go!(error; emit '<'; to Data; reconsume),
                }
            },

            states::EndTagOpen => match c {
                '>' => go!(error; to Data),
                _ => match ascii_letter(c) {
                    Some(cl) => go!(create_tag EndTag cl; to TagName),
                    None     => go!(error; to BogusComment),
                }
            },

            states::TagName => match c {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BeforeAttributeName),
                '/'  => go!(to SelfClosingStartTag),
                '>'  => go!(emit_tag; to Data),
                '\0' => go!(error; append_tag '\ufffd'),
                _    => go!(append_tag (ascii_letter(c).unwrap_or(c))),
            },

            // kind = ScriptDataEscaped
            states::RawLessThanSign(ScriptDataEscaped) => match c {
                '/' => go!(clear_temp; to_raw RawEndTagOpen ScriptDataEscaped),
                _ => match ascii_letter(c) {
                    Some(cl) => go!(clear_temp; append_temp cl;
                                    to ScriptDataDoubleEscapeStart; emit '<'; emit c),
                    None => go!(to_raw RawData ScriptDataEscaped; emit '<'; reconsume),
                }
            },

            // otherwise
            states::RawLessThanSign(kind) => match c {
                '/' => go!(clear_temp; to_raw RawEndTagOpen kind),
                '!' if kind == ScriptData => go!(to ScriptDataEscapeStart; emit '<'; emit '!'),
                _   => go!(to_raw RawData Rcdata; emit '<'; reconsume),
            },

            states::RawEndTagOpen(kind) => match ascii_letter(c) {
                Some(cl) => go!(create_tag EndTag cl; append_temp c; to_raw RawEndTagName kind),
                None     => go!(to_raw RawData kind; emit '<'; emit '/'; reconsume),
            },

            states::RawEndTagName(kind) => {
                if self.have_appropriate_end_tag() {
                    match c {
                        '\t' | '\n' | '\x0C' | ' '
                            => go!(to BeforeAttributeName; finish),
                        '/' => go!(to SelfClosingStartTag; finish),
                        '>' => go!(emit_tag; to Data; finish),
                        // All of the above end with a return from this function.

                        _ => (),
                    }
                }

                match ascii_letter(c) {
                    Some(cl) => go!(append_tag cl; append_temp c),
                    None     => go!(emit '<'; emit '/'; emit_temp; to_raw RawData kind; reconsume),
                }
            },

            states::ScriptDataEscapeStart => match c {
                '-' => go!(to ScriptDataEscapeStartDash; emit '-'),
                _   => go!(to_raw RawData ScriptData; reconsume),
            },

            states::ScriptDataEscapeStartDash => match c {
                '-' => go!(to ScriptDataEscapedDashDash; emit '-'),
                _   => go!(to_raw RawData ScriptData; reconsume),
            },

            states::ScriptDataEscapedDash => match c {
                '-'  => go!(to ScriptDataEscapedDashDash; emit '-'),
                '<'  => go!(to_raw RawLessThanSign ScriptDataEscaped),
                '\0' => go!(error; to_raw RawData ScriptDataEscaped; emit '\ufffd'),
                _    => go!(to_raw RawData ScriptDataEscaped; emit c),
            },

            states::ScriptDataEscapedDashDash => match c {
                '-'  => go!(emit '-'),
                '<'  => go!(to_raw RawLessThanSign ScriptDataEscaped),
                '>'  => go!(to_raw RawData ScriptData; emit '>'),
                '\0' => go!(error; to_raw RawData ScriptDataEscaped; emit '\ufffd'),
                _    => go!(to_raw RawData ScriptDataEscaped; emit c),
            },

            states::ScriptDataDoubleEscapeStart |
            states::ScriptDataDoubleEscaped |
            states::ScriptDataDoubleEscapedDash |
            states::ScriptDataDoubleEscapedDashDash |
            states::ScriptDataDoubleEscapedLessThanSign |
            states::ScriptDataDoubleEscapeEnd |
            states::BeforeAttributeName |
            states::AttributeName |
            states::AfterAttributeName |
            states::BeforeAttributeValue |
            states::AttributeValueDoubleQuoted |
            states::AttributeValueSingleQuoted |
            states::AttributeValueUnquoted |
            states::AfterAttributeValueQuoted |
            states::SelfClosingStartTag |
            states::CommentStart |
            states::CommentStartDash |
            states::Comment |
            states::CommentEndDash |
            states::CommentEnd |
            states::CommentEndBang |
            states::Doctype |
            states::BeforeDoctypeName |
            states::DoctypeName |
            states::AfterDoctypeName |
            states::AfterDoctypePublicKeyword |
            states::BeforeDoctypePublicIdentifier |
            states::DoctypePublicIdentifierDoubleQuoted |
            states::DoctypePublicIdentifierSingleQuoted |
            states::AfterDoctypePublicIdentifier |
            states::BetweenDoctypePublicAndSystemIdentifiers |
            states::AfterDoctypeSystemKeyword |
            states::BeforeDoctypeSystemIdentifier |
            states::DoctypeSystemIdentifierDoubleQuoted |
            states::DoctypeSystemIdentifierSingleQuoted |
            states::AfterDoctypeSystemIdentifier |
            states::BogusDoctype |
            states::CharacterReferenceInData |
            states::CharacterReferenceInRcdata |
            states::CharacterReferenceInAttributeValue |
            states::BogusComment |
            states::MarkupDeclarationOpen |
            states::CdataSection
                => fail!("FIXME: state {:?} not implemented", self.state),
        }

        Finished

    }
}
