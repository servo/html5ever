/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

pub use self::tokens::{Doctype, Attribute, TagKind, StartTag, EndTag, Tag, Token};
pub use self::tokens::{DoctypeToken, TagToken, CommentToken, CharacterToken};

use self::states::{Escaped, DoubleEscaped};
use self::states::{RawLessThanSign, RawEndTagOpen, RawEndTagName};
use self::states::{Rcdata, ScriptData, ScriptDataEscaped};

use std::util::replace;

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

    /// Current attribute.
    priv current_attr: Attribute,

    /// Current comment.
    priv current_comment: ~str,

    /// Last start tag name, for use in checking "appropriate end tag".
    priv last_start_tag_name: Option<~str>,

    /// The "temporary buffer" mentioned in the spec.
    priv temp_buf: ~str,

    /// The "additional allowed character" for character references.
    priv addnl_allowed: Option<char>,
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
            current_attr: Attribute::new(),
            current_comment: ~"",
            last_start_tag_name: None,
            temp_buf: ~"",
            addnl_allowed: None,
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
        self.finish_attribute();

        let tag = self.current_tag.take().unwrap();
        match tag.kind {
            StartTag => self.last_start_tag_name = Some(tag.name.clone()),
            _ => ()
        }
        self.sink.process_token(TagToken(tag));
    }

    fn emit_temp_buf(&mut self) {
        // FIXME: Add a multi-character token and move temp_buf into it, like
        // emit_current_comment below.
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

    fn emit_current_comment(&mut self) {
        self.sink.process_token(CommentToken(
            replace(&mut self.current_comment, ~"")));
    }

    fn create_tag(&mut self, kind: TagKind, c: char) {
        assert!(self.current_tag.is_none());
        let mut t = Tag::new(kind);
        t.name.push_char(c);
        self.current_tag = Some(t);
    }

    fn tag<'t>(&'t self) -> &'t Tag {
        // Only use this from places where the state machine guarantees we have a tag
        self.current_tag.get_ref()
    }

    fn tag_mut<'t>(&'t mut self) -> &'t mut Tag {
        self.current_tag.get_mut_ref()
    }

    fn have_appropriate_end_tag(&self) -> bool {
        match (self.last_start_tag_name.as_ref(), self.current_tag.as_ref()) {
            (Some(last), Some(tag)) =>
                (tag.kind == EndTag) && (tag.name.as_slice() == last.as_slice()),
            _ => false
        }
    }

    fn create_attribute(&mut self, c: char) {
        self.finish_attribute();

        let attr = &mut self.current_attr;
        attr.name.push_char(c);
    }

    fn finish_attribute(&mut self) {
        if self.current_attr.name.len() == 0 {
            return;
        }

        // Check for a duplicate attribute.
        // FIXME: the spec says we should error as soon as the name is finished.
        // FIXME: linear time search, do we care?
        let dup = {
            let name = self.current_attr.name.as_slice();
            self.tag().attrs.iter().any(|a| a.name.as_slice() == name)
        };

        if dup {
            error!("Parse error: duplicate attribute");
            self.current_attr.clear();
        } else {
            let attr = replace(&mut self.current_attr, Attribute::new());
            self.tag_mut().attrs.push(attr);
        }
    }
}

// Shorthand for common state machine behaviors.
macro_rules! shorthand (
    ( to $s:ident                     ) => ( self.state = states::$s;                              );
    ( to $s:ident $k1:expr            ) => ( self.state = states::$s($k1);                         );
    ( to $s:ident $k1:expr $k2:expr   ) => ( self.state = states::$s($k1($k2));                    );
    ( emit $c:expr                    ) => ( self.emit_char($c);                                   );
    ( create_tag $kind:expr $c:expr   ) => ( self.create_tag($kind, $c);                           );
    ( push_tag $c:expr                ) => ( self.tag_mut().name.push_char($c);                    );
    ( emit_tag                        ) => ( self.emit_current_tag();                              );
    ( push_temp $c:expr               ) => ( self.temp_buf.push_char($c);                          );
    ( emit_temp                       ) => ( self.emit_temp_buf();                                 );
    ( clear_temp                      ) => ( self.clear_temp_buf();                                );
    ( create_attr $c:expr             ) => ( self.create_attribute($c);                            );
    ( push_name $c:expr               ) => ( self.current_attr.name.push_char($c);                 );
    ( push_value $c:expr              ) => ( self.current_attr.value.push_char($c);                );
    ( addnl_allowed $c:expr           ) => ( self.addnl_allowed = Some($c);                        );
    ( no_addnl_allowed                ) => ( self.addnl_allowed = None;                            );
    ( push_comment $c:expr            ) => ( self.current_comment.push_char($c);                   );
    ( append_comment $c:expr          ) => ( self.current_comment.push_str($c);                    );
    ( emit_comment                    ) => ( self.emit_current_comment();                          );
    ( error                           ) => ( self.parse_error(c); /* capture! */                   );
)

// Tracing of tokenizer actions.  This adds significant bloat and compile time,
// so it's behind a cfg flag.
#[cfg(trace_tokenizer)]
macro_rules! step ( ( $($cmds:tt)* ) => ({
    debug!("  {:s}", stringify!($($cmds)*));
    shorthand!($($cmds)*);
}))

#[cfg(not(trace_tokenizer))]
macro_rules! step ( ( $($cmds:tt)* ) => ( shorthand!($($cmds)*) ) )

// A little DSL for sequencing shorthand actions.
macro_rules! go (
    // A pattern like $($cmd:tt)* ; $($rest:tt)* causes parse ambiguity.
    // We have to tell the parser how much lookahead we need.
    ( $a:tt                   ; $($rest:tt)* ) => ({ step!($a);          go!($($rest)*); });
    ( $a:tt $b:tt             ; $($rest:tt)* ) => ({ step!($a $b);       go!($($rest)*); });
    ( $a:tt $b:tt $c:tt       ; $($rest:tt)* ) => ({ step!($a $b $c);    go!($($rest)*); });
    ( $a:tt $b:tt $c:tt $d:tt ; $($rest:tt)* ) => ({ step!($a $b $c $d); go!($($rest)*); });

    // These can only come at the end.
    // FIXME: Come up with a better name for 'finish'.
    ( reconsume ) => ( return Reconsume; );
    ( finish    ) => ( return Finished;  );

    // If nothing else matched, it's a single command
    ( $($cmd:tt)+ ) => ( step!($($cmd)+); );

    // or nothing.
    () => (());
)

macro_rules! go_match ( ( $x:expr, $($pats:pat)|+ => $($cmds:tt)* ) => (
    match $x {
        $($pats)|+ => go!($($cmds)*),
        _ => (),
    }
))

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
            states::RawData(kind) => match (c, kind) {
                ('&', Rcdata) => go!(to CharacterReferenceInRcdata),
                ('-', ScriptDataEscaped(esc_kind)) => go!(to ScriptDataEscapedDash esc_kind; emit '-'),
                ('<', ScriptDataEscaped(DoubleEscaped)) => go!(to RawLessThanSign kind; emit '<'),
                ('<',  _) => go!(to RawLessThanSign kind),
                ('\0', _) => go!(error; emit '\ufffd'),
                _         => go!(emit c),
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
                '\0' => go!(error; push_tag '\ufffd'),
                _    => go!(push_tag (ascii_letter(c).unwrap_or(c))),
            },

            states::RawLessThanSign(ScriptDataEscaped(Escaped)) => match c {
                '/' => go!(clear_temp; to RawEndTagOpen ScriptDataEscaped Escaped),
                _ => match ascii_letter(c) {
                    Some(cl) => go!(clear_temp; push_temp cl;
                                    to ScriptDataEscapeStart DoubleEscaped; emit '<'; emit c),
                    None => go!(to RawData ScriptDataEscaped Escaped; emit '<'; reconsume),
                }
            },

            states::RawLessThanSign(ScriptDataEscaped(DoubleEscaped)) => match c {
                '/' => go!(clear_temp; to RawEndTagOpen ScriptDataEscaped DoubleEscaped),
                _   => go!(to RawData ScriptDataEscaped DoubleEscaped; reconsume),
            },

            // otherwise
            states::RawLessThanSign(kind) => match c {
                '/' => go!(clear_temp; to RawEndTagOpen kind),
                '!' if kind == ScriptData => go!(to ScriptDataEscapeStart Escaped; emit '<'; emit '!'),
                _   => go!(to RawData Rcdata; emit '<'; reconsume),
            },

            states::RawEndTagOpen(kind) => match ascii_letter(c) {
                Some(cl) => go!(create_tag EndTag cl; push_temp c; to RawEndTagName kind),
                None     => go!(to RawData kind; emit '<'; emit '/'; reconsume),
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
                    Some(cl) => go!(push_tag cl; push_temp c),
                    None     => go!(emit '<'; emit '/'; emit_temp; to RawData kind; reconsume),
                }
            },

            states::ScriptDataEscapeStart(DoubleEscaped) => match c {
                '\t' | '\n' | '\x0C' | ' ' | '/' | '>' => {
                    let esc = if self.temp_buf.as_slice() == "script" { DoubleEscaped } else { Escaped };
                    go!(to RawData ScriptDataEscaped esc; emit c);
                }

                _ => match ascii_letter(c) {
                    Some(cl) => go!(push_temp cl; emit c),
                    None     => go!(to RawData ScriptDataEscaped Escaped; reconsume),
                }
            },

            states::ScriptDataEscapeStart(Escaped) => match c {
                '-' => go!(to ScriptDataEscapeStartDash; emit '-'),
                _   => go!(to RawData ScriptData; reconsume),
            },

            states::ScriptDataEscapeStartDash => match c {
                '-' => go!(to ScriptDataEscapedDashDash Escaped; emit '-'),
                _   => go!(to RawData ScriptData; reconsume),
            },

            states::ScriptDataEscapedDash(kind) => match c {
                '-'  => go!(to ScriptDataEscapedDashDash kind; emit '-'),
                '<'  => {
                    go!(to RawLessThanSign ScriptDataEscaped kind);
                    if kind == DoubleEscaped { go!(emit '<'); }
                }
                '\0' => go!(error; to RawData ScriptDataEscaped kind; emit '\ufffd'),
                _    => go!(to RawData ScriptDataEscaped kind; emit c),
            },

            states::ScriptDataEscapedDashDash(kind) => match c {
                '-'  => go!(emit '-'),
                '<'  => {
                    go!(to RawLessThanSign ScriptDataEscaped kind);
                    if kind == DoubleEscaped { go!(emit '<'); }
                }
                '>'  => go!(to RawData ScriptData; emit '>'),
                '\0' => go!(error; to RawData ScriptDataEscaped kind; emit '\ufffd'),
                _    => go!(to RawData ScriptDataEscaped kind; emit c),
            },

            states::ScriptDataDoubleEscapeEnd => match c {
                '\t' | '\n' | '\x0C' | ' ' | '/' | '>' => {
                    let esc = if self.temp_buf.as_slice() == "script" { Escaped } else { DoubleEscaped };
                    go!(to RawData ScriptDataEscaped esc; emit c);
                }

                _ => match ascii_letter(c) {
                    Some(cl) => go!(push_temp cl; emit c),
                    None     => go!(to RawData ScriptDataEscaped DoubleEscaped; reconsume),
                }
            },

            states::BeforeAttributeName => match c {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '/'  => go!(to SelfClosingStartTag),
                '>'  => go!(to Data; emit_tag),
                '\0' => go!(error; create_attr '\ufffd'; to AttributeName),
                _    => match ascii_letter(c) {
                    Some(cl) => go!(create_attr cl; to AttributeName),
                    None => {
                        go_match!(c,
                            '"' | '\'' | '<' | '=' => error);
                        go!(create_attr c; to AttributeName);
                    }
                }
            },

            states::AttributeName => match c {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to AfterAttributeName),
                '/'  => go!(to SelfClosingStartTag),
                '='  => go!(to BeforeAttributeValue),
                '>'  => go!(to Data; emit_tag),
                '\0' => go!(error; push_name '\ufffd'),
                _    => match ascii_letter(c) {
                    Some(cl) => go!(push_name cl),
                    None => {
                        go_match!(c,
                            '"' | '\'' | '<' => error);
                        go!(push_name c);
                    }
                }
            },

            states::AfterAttributeName => match c {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '/'  => go!(to SelfClosingStartTag),
                '='  => go!(to BeforeAttributeValue),
                '>'  => go!(to Data; emit_tag),
                '\0' => go!(error; create_attr '\ufffd'; to AttributeName),
                _    => match ascii_letter(c) {
                    Some(cl) => go!(create_attr cl; to AttributeName),
                    None => {
                        go_match!(c,
                            '"' | '\'' | '<' => error);
                        go!(create_attr c; to AttributeName);
                    }
                }
            },

            states::BeforeAttributeValue => match c {
                '\t' | '\n' | '\x0C' | ' ' => (),
                '"'  => go!(to AttributeValueDoubleQuoted),
                '&'  => go!(to AttributeValueUnquoted; reconsume),
                '\'' => go!(to AttributeValueSingleQuoted),
                '\0' => go!(error; push_value '\ufffd'; to AttributeValueUnquoted),
                '>'  => go!(error; to Data; emit_tag),
                '<' | '=' | '`'
                     => go!(error; push_value c; to AttributeValueUnquoted),
                _    => go!(push_value c; to AttributeValueUnquoted),
            },

            states::AttributeValueDoubleQuoted => match c {
                '"'  => go!(to AfterAttributeValueQuoted),
                '&'  => go!(to CharacterReferenceInAttributeValue; addnl_allowed '"'),
                '\0' => go!(error; push_value '\ufffd'),
                _    => go!(push_value c),
            },

            states::AttributeValueSingleQuoted => match c {
                '\'' => go!(to AfterAttributeValueQuoted),
                '&'  => go!(to CharacterReferenceInAttributeValue; addnl_allowed '\''),
                '\0' => go!(error; push_value '\ufffd'),
                _    => go!(push_value c),
            },

            states::AttributeValueUnquoted => match c {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BeforeAttributeName),
                '&'  => go!(to CharacterReferenceInAttributeValue; addnl_allowed '>'),
                '>'  => go!(to Data; emit_tag),
                '\0' => go!(error; push_value '\ufffd'),
                _    => {
                    go_match!(c,
                        '"' | '\'' | '<' | '=' | '`' => error);
                    go!(push_value c);
                }
            },

            states::AfterAttributeValueQuoted => match c {
                '\t' | '\n' | '\x0C' | ' '
                     => go!(to BeforeAttributeName),
                '/'  => go!(to SelfClosingStartTag),
                '>'  => go!(to Data; emit_tag),
                _    => go!(error; to BeforeAttributeName; reconsume),
            },

            states::SelfClosingStartTag => match c {
                '>' => {
                    self.tag_mut().self_closing = true;
                    go!(to Data; emit_tag);
                }
                _ => go!(error; to BeforeAttributeName; reconsume),
            },

            states::CommentStart => match c {
                '-'  => go!(to CommentStartDash),
                '\0' => go!(error; push_comment '\ufffd'; to Comment),
                '>'  => go!(error; to Data; emit_comment),
                _    => go!(push_comment c; to Comment),
            },

            states::CommentStartDash => match c {
                '-'  => go!(to CommentEnd),
                '\0' => go!(error; append_comment "-\ufffd"; to Comment),
                '>'  => go!(error; to Data; emit_comment),
                _    => go!(push_comment '-'; push_comment c; to Comment),
            },

            states::Comment => match c {
                '-'  => go!(to CommentEndDash),
                '\0' => go!(error; push_comment '\ufffd'),
                _    => go!(push_comment c),
            },

            states::CommentEndDash => match c {
                '-'  => go!(to CommentEnd),
                '\0' => go!(error; append_comment "-\ufffd"; to Comment),
                _    => go!(push_comment '-'; push_comment c; to Comment),
            },

            states::CommentEnd => match c {
                '>'  => go!(to Data; emit_comment),
                '\0' => go!(append_comment "--\ufffd"; to Comment),
                '!'  => go!(error; to CommentEndBang),
                '-'  => go!(error; push_comment '-'),
                _    => go!(error; append_comment "--"; push_comment c; to Comment),
            },

            states::CommentEndBang => match c {
                '-'  => go!(append_comment "--!"; to CommentEndDash),
                '>'  => go!(to Data; emit_comment),
                '\0' => go!(error; append_comment "--!\ufffd"; to Comment),
                _    => go!(append_comment "--!"; push_comment c; to Comment),
            },

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
