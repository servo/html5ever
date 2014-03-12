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
                            // reconsume
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
    ( to          $state:ident         ) => ( self.state = states::$state; );
    ( emit        $c:expr              ) => ( self.emit_char($c);          );
    ( create_tag  $kind:expr   $c:expr ) => ( self.create_tag($kind, $c);  );
    ( append_tag  $c:expr              ) => ( self.append_to_tag_name($c); );
    ( emit_tag                         ) => ( self.emit_current_tag();     );
    ( append_temp $c:expr              ) => ( self.temp_buf.push_char($c); );
    ( emit_temp                        ) => ( self.emit_temp_buf();        );
    ( clear_temp                       ) => ( self.clear_temp_buf();       );

    // NB: Deliberate capture of 'c'
    ( error                            ) => ( self.parse_error(c);         );
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

            states::Rcdata => match c {
                '&'  => go!(to CharacterReferenceInRcdata),
                '<'  => go!(to RcdataLessThanSign),
                '\0' => go!(error; emit '\ufffd'),
                _    => go!(emit c),
            },

            states::Rawtext => match c {
                '<'  => go!(to RawtextLessThanSign),
                '\0' => go!(error; emit '\ufffd'),
                _    => go!(emit c),
            },

            states::ScriptData => match c {
                '<'  => go!(to ScriptDataLessThanSign),
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

            states::RcdataLessThanSign => match c {
                '/' => go!(clear_temp; to RcdataEndTagOpen),
                _   => go!(to Rcdata; emit '<'; reconsume),
            },

            states::RcdataEndTagOpen => match ascii_letter(c) {
                Some(cl) => go!(create_tag EndTag cl; append_temp c; to RcdataEndTagName),
                None     => go!(to Rcdata; emit '<'; emit '/'; reconsume),
            },

            states::RcdataEndTagName => {
                if self.have_appropriate_end_tag() {
                    match c {
                        '\t' | '\n' | '\x0C' | ' '
                            => go!(to BeforeAttributeName; finish),
                        '/' => go!(to SelfClosingStartTag; finish),
                        '>' => go!(emit_tag; to Data; finish),
                        _ => (),
                    }
                }

                match ascii_letter(c) {
                    Some(cl) => go!(append_tag cl; append_temp c),
                    None     => go!(emit '<'; emit '/'; emit_temp; to Rcdata; reconsume),
                }
            },

            states::RawtextLessThanSign => match c {
                '/' => go!(clear_temp; to RawtextEndTagOpen),
                _   => go!(to Rawtext; emit '<'; reconsume),
            },

            states::RawtextEndTagOpen => match ascii_letter(c) {
                Some(cl) => go!(create_tag EndTag cl; append_temp c; to RawtextEndTagName),
                None     => go!(to Rawtext; emit '<'; emit '/'; reconsume),
            },

            s => fail!("FIXME: state {:?} not implemented", s),
        }

        Finished

    }
}
