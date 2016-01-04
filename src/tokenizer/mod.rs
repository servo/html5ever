mod buffer_queue;
mod char_ref;
mod interface;
mod qname;
pub mod states;

pub use self::interface::{Attribute, Doctype, Pi};
pub use self::interface::{StartTag, EndTag, EmptyTag, ShortTag, QName};
pub use self::interface::{DoctypeToken, TagToken, PIToken, CommentToken};
pub use self::interface::{CharacterTokens, EOFToken, NullCharacterToken};
pub use self::interface::{TokenSink, ParseError, TagKind, Token, Tag};

use std::borrow::Cow::{self, Borrowed};
use std::ascii::AsciiExt;
use std::collections::{BTreeMap};
use std::mem::replace;

use string_cache::{Atom};
use tendril::StrTendril;

use self::buffer_queue::{BufferQueue, SetResult, FromSet, NotFromSet};
use self::char_ref::{CharRefTokenizer, CharRef};
use self::states::{Unquoted, SingleQuoted, DoubleQuoted};
use self::states::{Data, TagState, XmlState};
use self::states::{DoctypeKind, Public, System};
use self::qname::{QNameTokenizer};
use util::smallcharset::SmallCharSet;



/// Copy of Tokenizer options, with an impl for `Default`.
#[derive(Copy, Clone)]
pub struct XmlTokenizerOpts {
    /// Report all parse errors described in the spec, at some
    /// performance penalty?  Default: false
    pub exact_errors: bool,

    /// Discard a `U+FEFF BYTE ORDER MARK` if we see one at the beginning
    /// of the stream?  Default: true
    pub discard_bom: bool,

    /// Keep a record of how long we spent in each state?  Printed
    /// when `end()` is called.  Default: false
    pub profile: bool,

    /// Initial state override.  Only the test runner should use
    /// a non-`None` value!
    pub initial_state: Option<states::XmlState>,

}

fn process_qname(tag_name: StrTendril) -> QName {
    // If tag name can't possibly contain full namespace, skip qualified name
    // parsing altogether. For a tag to have namespace it must look like:
    //     a:b
    // Since StrTendril are UTF-8, we know that minimal size in bytes must be
    // three bytes minimum.
    let split = if (&*tag_name).as_bytes().len() <3 {
        None
    } else {
        QNameTokenizer::new((&*tag_name).as_bytes()).run()
    };

    match split {
        None => QName::new(Atom::from(""), Atom::from(&*tag_name)),
        Some(col) => {
            let len = (&*tag_name).as_bytes().len() as u32;
            let prefix = tag_name.subtendril(0, col);
            let local =  tag_name.subtendril(col+1, len - col -1);
            QName::new(Atom::from(&*prefix), Atom::from(&*local))
        },
    }
}

fn option_push(opt_str: &mut Option<StrTendril>, c: char) {
    match *opt_str {
        Some(ref mut s) => s.push_char(c),
        None => *opt_str = Some(StrTendril::from_char(c)),
    }
}

impl Default for XmlTokenizerOpts {
    fn default() -> XmlTokenizerOpts {
        XmlTokenizerOpts {
            exact_errors: false,
            discard_bom: true,
            profile: false,
            initial_state: None,
        }
    }
}
/// The Xml tokenizer.
pub struct XmlTokenizer<Sink> {
    /// Options controlling the behavior of the tokenizer.
    opts: XmlTokenizerOpts,

    /// Destination for tokens we emit.
    sink: Sink,

    /// The abstract machine state as described in the spec.
    state: states::XmlState,

    /// Input ready to be tokenized.
    input_buffers: BufferQueue,

    /// Are we at the end of the file, once buffers have been processed
    /// completely? This affects whether we will wait for lookahead or not.
    at_eof: bool,

    /// Tokenizer for character references, if we're tokenizing
    /// one at the moment.
    char_ref_tokenizer: Option<Box<CharRefTokenizer>>,

    /// Current input character.  Just consumed, may reconsume.
    current_char: char,

    /// Should we reconsume the current input character?
    reconsume: bool,

    /// Did we just consume \r, translating it to \n?  In that case we need
    /// to ignore the next character if it's \n.
    ignore_lf: bool,

    /// Discard a U+FEFF BYTE ORDER MARK if we see one?  Only done at the
    /// beginning of the stream.
    discard_bom: bool,

    /// Current tag kind.
    current_tag_kind: TagKind,

    /// Current tag name.
    current_tag_name: StrTendril,

    /// Current tag attributes.
    current_tag_attrs: Vec<Attribute>,

    /// Current attribute name.
    current_attr_name: StrTendril,

    /// Current attribute value.
    current_attr_value: StrTendril,

    current_doctype: Doctype,

    /// Current comment.
    current_comment: StrTendril,

    /// Current processing instruction target.
    current_pi_target: StrTendril,

    /// Current processing instruction value.
    current_pi_data: StrTendril,

    /// Record of how many ns we spent in each state, if profiling is enabled.
    state_profile: BTreeMap<states::XmlState, u64>,

    /// Record of how many ns we spent in the token sink.
    time_in_sink: u64,
}

impl <Sink:TokenSink> XmlTokenizer<Sink> {
    /// Create a new tokenizer which feeds tokens to a particular `TokenSink`.
    pub fn new(sink: Sink, opts: XmlTokenizerOpts) -> XmlTokenizer<Sink> {
        if opts.profile && cfg!(for_c) {
            panic!("Can't profile tokenizer when built as a C library");
        }

        let state = *opts.initial_state.as_ref().unwrap_or(&states::Data);
        let discard_bom = opts.discard_bom;
        XmlTokenizer {
            opts: opts,
            sink: sink,
            state: state,
            char_ref_tokenizer: None,
            input_buffers: BufferQueue::new(),
            at_eof: false,
            current_char: '\0',
            reconsume: false,
            ignore_lf: false,
            discard_bom: discard_bom,
            current_tag_kind: StartTag,
            current_tag_name: StrTendril::new(),
            current_tag_attrs: vec!(),
            current_attr_name: StrTendril::new(),
            current_attr_value: StrTendril::new(),
            current_comment: StrTendril::new(),
            current_pi_data: StrTendril::new(),
            current_pi_target: StrTendril::new(),
            current_doctype: Doctype::new(),
            state_profile: BTreeMap::new(),
            time_in_sink: 0,
        }
    }
    pub fn unwrap(self) -> Sink {
        self.sink
    }

    pub fn sink<'a>(&'a self) -> &'a Sink {
        &self.sink
    }

    pub fn sink_mut<'a>(&'a mut self) -> &'a mut Sink {
        &mut self.sink
    }

    /// Feed an input string into the tokenizer.
    pub fn feed(&mut self, mut input: StrTendril) {
        if input.len() == 0 {
            return;
        }

        if self.discard_bom && input.starts_with("\u{FFEF}") {
            input.pop_front(3);  // length of BOM in UTF-8
        }

        self.input_buffers.push_back(input);
        self.run();
    }

    fn process_token(&mut self, token: Token) {
        if self.opts.profile {
            let (_, dt) = time!(self.sink.process_token(token));
            self.time_in_sink += dt;
        } else {
            self.sink.process_token(token);
        }
    }

    // Get the next input character, which might be the character
    // 'c' that we already consumed from the buffers.
    fn get_preprocessed_char(&mut self, mut c: char) -> Option<char> {
        if self.ignore_lf {
            self.ignore_lf = false;
            if c == '\n' {
                c = unwrap_or_return!(self.input_buffers.next(), None);
            }
        }

        if c == '\r' {
            self.ignore_lf = true;
            c = '\n';
        }

        // Normalize \x00 into \uFFFD
        if c == '\x00' {
            c = '\u{FFFD}'
        }

        // Exclude forbidden Unicode characters
        if self.opts.exact_errors && match c as u32 {
            0x01...0x08 | 0x0B | 0x0E...0x1F | 0x7F...0x9F | 0xFDD0...0xFDEF => true,
            n if (n & 0xFFFE) == 0xFFFE => true,
            _ => false,
        } {
            let msg = format!("Bad character {}", c);
            self.emit_error(Cow::Owned(msg));
        }

        debug!("got character {}", c);
        self.current_char = c;
        Some(c)
    }

    fn bad_eof_error(&mut self) {
        let msg = format_if!(
            self.opts.exact_errors,
            "Unexpected EOF",
            "Saw EOF in state {:?}", self.state);
        self.emit_error(msg);
    }

    fn pop_except_from(&mut self, set: SmallCharSet) -> Option<SetResult> {
        // Bail to the slow path for various corner cases.
        // This means that `FromSet` can contain characters not in the set!
        // It shouldn't matter because the fallback `FromSet` case should
        // always do the same thing as the `NotFromSet` case.
        if self.opts.exact_errors || self.reconsume || self.ignore_lf {
            return self.get_char().map(|x| FromSet(x));
        }

        let d = self.input_buffers.pop_except_from(set);
        debug!("got characters {:?}", d);
        match d {
            Some(FromSet(c)) => self.get_preprocessed_char(c).map(|x| FromSet(x)),

            // NB: We don't set self.current_char for a run of characters not
            // in the set.  It shouldn't matter for the codepaths that use
            // this.
            _ => d
        }
    }

    // Check if the next characters are an ASCII case-insensitive match.  See
    // BufferQueue::eat.
    //
    // NB: this doesn't do input stream preprocessing or set the current input
    // character.
    fn eat(&mut self, pat: &str) -> Option<bool> {
        match self.input_buffers.eat(pat) {
            None if self.at_eof => Some(false),
            r => r,
        }
    }

    // Run the state machine for as long as we can.
    fn run(&mut self) {
        if self.opts.profile {
            loop {
                let state = self.state;
                let old_sink = self.time_in_sink;
                let (run, mut dt) = time!(self.step());
                dt -= self.time_in_sink - old_sink;
                let new = match self.state_profile.get_mut(&state) {
                    Some(x) => {
                        *x += dt;
                        false
                    }
                    None => true,
                };
                if new {
                    // do this here because of borrow shenanigans
                    self.state_profile.insert(state, dt);
                }
                if !run { break; }
            }
        } else {
            while self.step() {
            }
        }
    }

    //§ tokenization
    // Get the next input character, if one is available.
    fn get_char(&mut self) -> Option<char> {
        if self.reconsume {
            self.reconsume = false;
            Some(self.current_char)
        } else {
            self.input_buffers.next()
                .and_then(|c| self.get_preprocessed_char(c))
        }
    }

    fn bad_char_error(&mut self) {
        let msg = format_if!(
            self.opts.exact_errors,
            "Bad character",
            "Saw {} in state {:?}", self.current_char, self.state);
        self.emit_error(msg);
    }

    fn discard_tag(&mut self) {
        self.current_tag_name = StrTendril::new();
        self.current_tag_attrs = Vec::new();
    }

    fn create_tag(&mut self, kind: TagKind, c: char) {
        self.discard_tag();
        self.current_tag_name.push_char(c);
        self.current_tag_kind = kind;
    }

    // This method creates a PI token and
    // sets its target to given char
    fn create_pi(&mut self, c: char) {
        self.current_pi_target = StrTendril::new();
        self.current_pi_data  = StrTendril::new();
        self.current_pi_target.push_char(c);
    }

    fn emit_char(&mut self, c: char) {
        self.process_token(CharacterTokens(StrTendril::from_char(match c {
            '\0' => '\u{FFFD}',
            c => c,
        })));
    }

    fn emit_short_tag(&mut self) {
        self.current_tag_kind = ShortTag;
        self.current_tag_name = StrTendril::new();
        self.emit_current_tag();
    }

    fn emit_empty_tag(&mut self) {
        self.current_tag_kind = EmptyTag;
        self.emit_current_tag();
    }

    fn set_empty_tag(&mut self) {
        self.current_tag_kind = EmptyTag;
    }

    fn emit_start_tag(&mut self) {
        self.current_tag_kind = StartTag;
        self.emit_current_tag();
    }


    fn emit_current_tag(&mut self) {

        self.finish_attribute();

        let qname = process_qname(replace(&mut self.current_tag_name, StrTendril::new()));


        match self.current_tag_kind {
            StartTag | EmptyTag => {},
            EndTag => {
                if !self.current_tag_attrs.is_empty() {
                    self.emit_error(Borrowed("Attribtes on an end tag"));
                }
            },
            ShortTag => {
                if !self.current_tag_attrs.is_empty() {
                    self.emit_error(Borrowed("Attributes on a short tag"));
                }
            },
        }

        let token = TagToken(Tag { kind: self.current_tag_kind,
            name: qname,
            attrs: replace(&mut self.current_tag_attrs, vec!()),
        });
        self.process_token(token);


        if self.current_tag_kind == StartTag {
            match self.sink.query_state_change() {
                None => (),
                Some(s) => self.state = s,
            }
        }
    }

    // The string must not contain '\0'!
    fn emit_chars(&mut self, b: StrTendril) {
        self.process_token(CharacterTokens(b));
    }

    // Emits the current Processing Instruction
    fn emit_pi(&mut self) {
        let token = PIToken(Pi {
            target: replace(&mut self.current_pi_target, StrTendril::new()),
            data: replace(&mut self.current_pi_data, StrTendril::new()),
        });
        self.process_token(token);
    }

    fn consume_char_ref(&mut self, addnl_allowed: Option<char>) {
        // NB: The char ref tokenizer assumes we have an additional allowed
        // character iff we're tokenizing in an attribute value.
        self.char_ref_tokenizer = Some(Box::new(CharRefTokenizer::new(addnl_allowed)));
    }

    fn emit_eof(&mut self) {
        self.process_token(EOFToken);
    }

    fn emit_error(&mut self, error: Cow<'static, str>) {
        self.process_token(ParseError(error));
    }


    fn emit_current_comment(&mut self) {
        let comment = replace(&mut self.current_comment, StrTendril::new());
        self.process_token(CommentToken(comment));
    }

    fn emit_current_doctype(&mut self) {
        let doctype = replace(&mut self.current_doctype, Doctype::new());
        self.process_token(DoctypeToken(doctype));
    }

    fn doctype_id<'a>(&'a mut self, kind: DoctypeKind) -> &'a mut Option<StrTendril> {
        match kind {
            Public => &mut self.current_doctype.public_id,
            System => &mut self.current_doctype.system_id,
        }
    }

    fn clear_doctype_id(&mut self, kind: DoctypeKind) {
        let id = self.doctype_id(kind);
        match *id {
            Some(ref mut s) => s.clear(),
            None => *id = Some(StrTendril::new()),
        }
    }

    fn peek(&mut self) -> Option<char> {
        if self.reconsume {
            Some(self.current_char)
        } else {
            self.input_buffers.peek()
        }
    }

    fn discard_char(&mut self) {
        let c = self.get_char();
        assert!(c.is_some());
    }

    fn unconsume(&mut self, buf: StrTendril) {
        self.input_buffers.push_front(buf);
    }
}

// Shorthand for common state machine behaviors.
macro_rules! shorthand (
    ( $me:ident : emit $c:expr                     ) => ( $me.emit_char($c);                                   );
    ( $me:ident : create_tag $kind:ident $c:expr   ) => ( $me.create_tag($kind, $c);                           );
    ( $me:ident : push_tag $c:expr                 ) => ( $me.current_tag_name.push_char($c);                  );
    ( $me:ident : discard_tag                      ) => ( $me.discard_tag();                                   );
    ( $me:ident : discard_char                     ) => ( $me.discard_char();                                  );
    ( $me:ident : push_temp $c:expr                ) => ( $me.temp_buf.push_char($c);                          );
    ( $me:ident : emit_temp                        ) => ( $me.emit_temp_buf();                                 );
    ( $me:ident : clear_temp                       ) => ( $me.clear_temp_buf();                                );
    ( $me:ident : create_attr $c:expr              ) => ( $me.create_attribute($c);                            );
    ( $me:ident : push_name $c:expr                ) => ( $me.current_attr_name.push_char($c);                 );
    ( $me:ident : push_value $c:expr               ) => ( $me.current_attr_value.push_char($c);                );
    ( $me:ident : append_value $c:expr             ) => ( $me.current_attr_value.push_tendril($c);             );
    ( $me:ident : push_comment $c:expr             ) => ( $me.current_comment.push_char($c);                   );
    ( $me:ident : append_comment $c:expr           ) => ( $me.current_comment.push_slice($c);                  );
    ( $me:ident : emit_comment                     ) => ( $me.emit_current_comment();                          );
    ( $me:ident : clear_comment                    ) => ( $me.current_comment.clear();                         );
    ( $me:ident : create_doctype                   ) => ( $me.current_doctype = Doctype::new();                );
    ( $me:ident : push_doctype_name $c:expr        ) => ( option_push(&mut $me.current_doctype.name, $c);      );
    ( $me:ident : push_doctype_id $k:ident $c:expr ) => ( option_push($me.doctype_id($k), $c);                 );
    ( $me:ident : clear_doctype_id $k:ident        ) => ( $me.clear_doctype_id($k);                            );
    ( $me:ident : emit_doctype                     ) => ( $me.emit_current_doctype();                          );
    ( $me:ident : error                            ) => ( $me.bad_char_error();                                );
    ( $me:ident : error_eof                        ) => ( $me.bad_eof_error();                                 );
    ( $me:ident : create_pi $c:expr                ) => ( $me.create_pi($c);                                   );
    ( $me:ident : push_pi_target $c:expr           ) => ( $me.current_pi_target.push_char($c);                 );
    ( $me:ident : push_pi_data $c:expr             ) => ( $me.current_pi_data.push_char($c);                   );
    ( $me:ident : set_empty_tag                    ) => ( $me.set_empty_tag();                                 );
);

// Tracing of tokenizer actions.  This adds significant bloat and compile time,
// so it's behind a cfg flag.
#[cfg(trace_tokenizer)]
macro_rules! sh_trace ( ( $me:ident : $($cmds:tt)* ) => ({
    debug!("  {:s}", stringify!($($cmds)*));
    shorthand!($me:expr : $($cmds)*);
}));

#[cfg(not(trace_tokenizer))]
macro_rules! sh_trace ( ( $me:ident : $($cmds:tt)* ) => ( shorthand!($me: $($cmds)*) ) );

// A little DSL for sequencing shorthand actions.
macro_rules! go (
    // A pattern like $($cmd:tt)* ; $($rest:tt)* causes parse ambiguity.
    // We have to tell the parser how much lookahead we need.

    ( $me:ident : $a:tt                   ; $($rest:tt)* ) => ({ sh_trace!($me: $a);          go!($me: $($rest)*); });
    ( $me:ident : $a:tt $b:tt             ; $($rest:tt)* ) => ({ sh_trace!($me: $a $b);       go!($me: $($rest)*); });
    ( $me:ident : $a:tt $b:tt $c:tt       ; $($rest:tt)* ) => ({ sh_trace!($me: $a $b $c);    go!($me: $($rest)*); });
    ( $me:ident : $a:tt $b:tt $c:tt $d:tt ; $($rest:tt)* ) => ({ sh_trace!($me: $a $b $c $d); go!($me: $($rest)*); });

    // These can only come at the end.

    ( $me:ident : to $s:ident                    ) => ({ $me.state = states::$s; return true;           });
    ( $me:ident : to $s:ident $k1:expr           ) => ({ $me.state = states::$s($k1); return true;      });
    ( $me:ident : to $s:ident $k1:ident $k2:expr ) => ({ $me.state = states::$s($k1($k2)); return true; });

    ( $me:ident : reconsume $s:ident                    ) => ({ $me.reconsume = true; go!($me: to $s);         });
    ( $me:ident : reconsume $s:ident $k1:expr           ) => ({ $me.reconsume = true; go!($me: to $s $k1);     });
    ( $me:ident : reconsume $s:ident $k1:ident $k2:expr ) => ({ $me.reconsume = true; go!($me: to $s $k1 $k2); });

    ( $me:ident : consume_char_ref             ) => ({ $me.consume_char_ref(None); return true;         });
    ( $me:ident : consume_char_ref $addnl:expr ) => ({ $me.consume_char_ref(Some($addnl)); return true; });

    // We have a default next state after emitting a tag, but the sink can override.
    ( $me:ident : emit_tag $s:ident ) => ({
        $me.state = states::$s;
        $me.emit_current_tag();
        return true;
    });

    // We have a special when dealing with empty and short tags in Xml
    ( $me:ident : emit_short_tag $s:ident ) => ({
        $me.state = states::$s;
        $me.emit_short_tag();
        return true;
    });

    ( $me:ident : emit_empty_tag $s:ident ) => ({
        $me.state = states::$s;
        $me.emit_empty_tag();
        return true;
    });

    ( $me:ident : emit_start_tag $s:ident ) => ({
        $me.state = states::$s;
        $me.emit_start_tag();
        return true;
    });

    ( $me:ident : emit_pi $s:ident ) => ({
        $me.state = states::$s;
        $me.emit_pi();
        return true;
    });

    ( $me:ident : eof ) => ({ $me.emit_eof(); return false; });

    // If nothing else matched, it's a single command
    ( $me:ident : $($cmd:tt)+ ) => ( sh_trace!($me: $($cmd)+); );

    // or nothing.
    ( $me:ident : ) => (());
);

macro_rules! go_match ( ( $me:ident : $x:expr, $($pats:pat),+ => $($cmds:tt)* ) => (
    match $x {
        $($pats)|+ => go!($me: $($cmds)*),
        _ => (),
    }
));

// This is a macro because it can cause early return
// from the function where it is used.
macro_rules! get_char ( ($me:expr) => (
    unwrap_or_return!($me.get_char(), false)
));

macro_rules! peek ( ($me:expr) => (
    unwrap_or_return!($me.peek(), false)
));

macro_rules! pop_except_from ( ($me:expr, $set:expr) => (
    unwrap_or_return!($me.pop_except_from($set), false)
));

macro_rules! eat ( ($me:expr, $pat:expr) => (
    unwrap_or_return!($me.eat($pat), false)
));


impl<Sink: TokenSink> XmlTokenizer<Sink> {

    // Run the state machine for a while.
    // Return true if we should be immediately re-invoked
    // (this just simplifies control flow vs. break / continue).
    fn step(&mut self) -> bool {
        if self.char_ref_tokenizer.is_some() {
            return self.step_char_ref_tokenizer();
        }

        println!("processing in state {:?}", self.state);
        match self.state {
            //§ data-state
            XmlState::Data => loop {
                match pop_except_from!(self, small_char_set!('\r' '&' '<')) {
                    FromSet('&')  => go!(self: consume_char_ref),
                    FromSet('<')  => go!(self: to TagState),
                    FromSet(c)    => go!(self: emit c),
                    NotFromSet(b) => self.emit_chars(b),
                }
            },
            //§ tag-state
            XmlState::TagState => loop { match get_char!(self) {
                '!' => go!(self: to MarkupDecl),
                '/' => go!(self: to EndTagState),
                '?' => go!(self: to Pi),
                '\t'| '\n' | ' '|
                ':' | '<' | '>' => go!(self: error; emit '<'; reconsume Data),
                cl => go!(self: create_tag StartTag cl; to TagName),
                }
            },
            //§ end-tag-state
            XmlState::EndTagState => loop { match get_char!(self) {
                '>' => go!(self:  emit_short_tag Data),
                '\t' | '\n' | ' '|
                '<' | ':'  => go!(self: error; emit '<'; emit '/'; reconsume Data),
                cl => go!(self: create_tag EndTag cl; to EndTagName)
                }
            },
            //§ end-tag-name-state
            XmlState::EndTagName => loop { match get_char!(self) {
                '\t' | '\n'
                | ' '   => go!(self: to EndTagNameAfter),
                '/'     => go!(self: error; to EndTagNameAfter),
                '>'     => go!(self: emit_tag Data),
                cl      => go!(self: push_tag cl),
                }
            },
            //§ end-tag-name-after-state
            XmlState::EndTagNameAfter => loop {match get_char!(self) {
                '>'     => go!(self: emit_tag Data),
                '\t' | '\n'
                | ' '   => (),
                _       => self.emit_error(Borrowed("Unexpected element in tag name")),
                }
            },
            //§ pi-state
            XmlState::Pi => loop { match get_char!(self) {
                '\t' | '\n'
                | ' '  => go!(self: error; reconsume BogusComment),
                cl     =>  go!(self: create_pi cl; to PiTarget),
                }
            },
            //§ pi-target-state
            XmlState::PiTarget => loop { match get_char!(self) {
                '\t' | '\n'
                | ' '  => go!(self: to PiTargetAfter),
                '?'    => go!(self: to PiAfter),
                cl     => go!(self: push_pi_target cl),
                }
            },
            //§ pi-target-after-state
            XmlState::PiTargetAfter => loop { match get_char!(self) {
                '\t' | '\n' | ' '  => (),
                _     => go!(self: reconsume PiData),
                }
            },
            //§ pi-data-state
            XmlState::PiData => loop { match get_char!(self) {
                '?' => go!(self: to PiAfter),
                cl  => go!(self: push_pi_data cl),
                }
            },
            //§ pi-after-state
            XmlState::PiAfter => loop { match get_char!(self) {
                '>' => go!(self: emit_pi Data),
                '?' => go!(self: to PiAfter),
                cl  => go!(self: push_pi_data cl),
                }
            },
            //§ markup-declaration-state
            XmlState::MarkupDecl => loop {
                if eat!(self, "--") {
                    go!(self: clear_comment; to Comment);
                } else if eat!(self, "[CDATA[") {
                    go!(self: to Cdata);
                } else if eat!(self, "DOCTYPE") {
                    go!(self: to Doctype);
                } else {
                    // FIXME: 'error' gives wrong message
                    go!(self: error; to BogusComment);
                }
            },
            //§ comment-state
            XmlState::Comment => loop { match get_char!(self) {
                '-' => go!(self: to CommentDash),
                '>' => go!(self: error; emit_comment; to Data),
                c   => go!(self: push_comment c; to Comment),
                }
            },
            //§ comment-dash-state
            XmlState::CommentDash => loop { match get_char!(self) {
                '-' => go!(self: to CommentEnd),
                c   => go!(self: push_comment c),
                }
            },
            //§ comment-end-state
            XmlState::CommentEnd => loop { match get_char!(self) {
                '>' => go!(self: emit_comment; to Data),
                '-' => go!(self: push_comment '-'),
                c   => go!(self: append_comment "--"; push_comment c; to Comment),
                }
            },
            //§ cdata-state
            XmlState::Cdata => loop { match get_char!(self) {
                    ']' => go!(self: to CdataBracket),
                    cl  => go!(self: emit cl),
                }
            },
            //§ cdata-bracket-state
            XmlState::CdataBracket => loop {  match get_char!(self) {
                    ']' => go!(self: to CdataEnd),
                    cl  => go!(self: emit ']'; emit cl; to Cdata),
                }
            },
            //§ cdata-end-state
            XmlState::CdataEnd => loop {  match get_char!(self) {
                '>' => go!(self: to Data),
                ']' => go!(self: emit ']'),
                cl  => go!(self: emit ']'; emit ']'; emit cl; to Cdata),
                }
            },
            //§ tag-name-state
            XmlState::TagName => loop { match get_char!(self) {
                '\t' | '\n'
                | ' '   => go!(self: to TagAttrNameBefore),
                '>'     => go!(self: emit_tag Data),
                '/'     => go!(self: set_empty_tag; to TagEmpty),
                cl      => go!(self: push_tag cl),
                }
            },
            //§ empty-tag-state
            XmlState::TagEmpty => loop { match get_char!(self) {
                '>'     => go!(self: emit_empty_tag Data),
                _       => go!(self: reconsume TagAttrValueBefore),
                }
            },
            //§ tag-attribute-name-before-state
            XmlState::TagAttrNameBefore => loop { match get_char!(self) {
                '\t' | '\n'
                | ' '   => (),
                '>'     => go!(self: emit_tag Data),
                '/'     => go!(self: set_empty_tag; to TagEmpty),
                ':'     => go!(self: error ),
                cl      => go!(self: create_attr cl; to TagAttrName),
                }
            },
            //§ tag-attribute-name-state
            XmlState::TagAttrName => loop { match get_char!(self) {
                '='     => go!(self: to TagAttrValueBefore),
                '>'     => go!(self: emit_tag Data),
                '\t' | '\n'
                | ' '   => go!(self: to TagAttrNameAfter),
                '/'     => go!(self: set_empty_tag; to TagEmpty),
                cl      => go!(self: push_name cl),
                }
            },
            //§ tag-attribute-name-after-state
            XmlState::TagAttrNameAfter => loop { match get_char!(self) {
                '\t' | '\n'
                | ' '   => (),
                '='     => go!(self: to TagAttrValueBefore),
                '>'     => go!(self: emit_tag Data),
                '/'     => go!(self: set_empty_tag; to TagEmpty),
                cl      => go!(self: create_attr cl; to TagAttrName),
                }
            },
            //§ tag-attribute-value-before-state
            XmlState::TagAttrValueBefore => loop { match get_char!(self) {
                '\t' | '\n'
                | ' '   => (),
                '"'     => go!(self: to TagAttrValue(DoubleQuoted)),
                '\''    => go!(self: to TagAttrValue(SingleQuoted)),
                '&'     => go!(self: reconsume TagAttrValue(Unquoted)),
                '>'     => go!(self: emit_tag Data),
                cl      => go!(self: push_value cl; to TagAttrValue(Unquoted)),
                }
            },
            //§ tag-attribute-value-double-quoted-state
            XmlState::TagAttrValue(DoubleQuoted) => loop {
                match pop_except_from!(self, small_char_set!('\n' '"' '&')) {
                    FromSet('"')        => go!(self: to TagAttrNameBefore),
                    FromSet('&')        => go!(self: consume_char_ref '"' ),
                    FromSet(c)          => go!(self: push_value c),
                    NotFromSet(ref b)   => go!(self: append_value b),
                }
            },
            //§ tag-attribute-value-single-quoted-state
            XmlState::TagAttrValue(SingleQuoted) => loop {
                match pop_except_from!(self, small_char_set!('\n' '\'' '&')) {
                    FromSet('\'')       => go!(self: to TagAttrNameBefore),
                    FromSet('&')        => go!(self: consume_char_ref '\''),
                    FromSet(c)          => go!(self: push_value c),
                    NotFromSet(ref b)   => go!(self: append_value b),
                }
            },
            //§ tag-attribute-value-double-quoted-state
            XmlState::TagAttrValue(Unquoted) => loop {
                match pop_except_from!(self, small_char_set!('\n' '\t' ' ' '&' '>')) {
                    FromSet('\t') | FromSet('\n') | FromSet(' ')
                     => go!(self: to TagAttrNameBefore),
                    FromSet('&')        => go!(self: consume_char_ref ),
                    FromSet('>')        => go!(self: emit_tag Data),
                    FromSet(c)          => go!(self: push_value c),
                    NotFromSet(ref b)   => go!(self: append_value b),
                }
            },
            //§ bogus-comment-state
            XmlState::BogusComment => loop { match get_char!(self) {
                '>'  => go!(self: emit_comment; to Data),
                c    => go!(self: push_comment c),
                }
            },
            //§ doctype-state
            XmlState::Doctype => loop { match get_char!(self) {
                '\t' | '\n' | '\x0C'
                | ' ' => go!(self: to BeforeDoctypeName),
                _     => go!(self: error; reconsume BeforeDoctypeName),
                }
            },
            //§ before-doctype-name-state
            XmlState::BeforeDoctypeName => loop { match get_char!(self) {
                '\t' | '\n' | '\x0C'
                | ' ' => (),
                '>'  => go!(self: error; emit_doctype; to Data),
                c    => go!(self: create_doctype; push_doctype_name (c.to_ascii_lowercase());
                                  to DoctypeName),
                }
            },
            //§ doctype-name-state
            XmlState::DoctypeName => loop { match get_char!(self) {
                '\t' | '\n' | '\x0C'
                | ' '   => go!(self: to AfterDoctypeName),
                '>'     => go!(self: emit_doctype; to Data),
                c       => go!(self: push_doctype_name (c.to_ascii_lowercase());
                                  to DoctypeName),
                }
            },
            //§ after-doctype-name-state
            XmlState::AfterDoctypeName => loop {
                if eat!(self, "public") {
                    go!(self: to AfterDoctypeKeyword Public);
                } else if eat!(self, "system") {
                    go!(self: to AfterDoctypeKeyword System);
                } else {
                    match get_char!(self) {
                        '\t' | '\n' | '\x0C' | ' ' => (),
                        '>' => go!(self: emit_doctype; to Data),
                        _   => go!(self: error; to BogusDoctype),
                    }
                }
            },
            //§ after-doctype-public-keyword-state
            XmlState::AfterDoctypeKeyword(Public) => loop { match get_char!(self) {
                '\t' | '\n' | '\x0C'
                | ' '   => go!(self: to BeforeDoctypeIdentifier Public),
                '"'     => go!(self: error; clear_doctype_id Public; to DoctypeIdentifierDoubleQuoted Public),
                '\''    => go!(self: error; clear_doctype_id Public; to DoctypeIdentifierSingleQuoted Public),
                '>'     => go!(self: error; emit_doctype; to Data),
                _       => go!(self: error; to BogusDoctype),
                }
            },
             //§ after-doctype-system-keyword-state
            XmlState::AfterDoctypeKeyword(System) => loop { match get_char!(self) {
                '\t' | '\n' | '\x0C'
                | ' '   => go!(self: to BeforeDoctypeIdentifier System),
                '"'     => go!(self: error; clear_doctype_id System; to DoctypeIdentifierDoubleQuoted System),
                '\''    => go!(self: error; clear_doctype_id System; to DoctypeIdentifierSingleQuoted System),
                '>'     => go!(self: error; emit_doctype; to Data),
                _       => go!(self: error; to BogusDoctype),
                }
            },
            //§ before_doctype_public_identifier_state before_doctype_system_identifier_state
            XmlState::BeforeDoctypeIdentifier(kind) => loop { match get_char!(self) {
                '\t' | '\n' | '\x0C'
                | ' '   => (),
                '"'     => go!(self: clear_doctype_id kind; to DoctypeIdentifierDoubleQuoted kind),
                '\''    => go!(self: clear_doctype_id kind; to DoctypeIdentifierSingleQuoted kind),
                '>'     => go!(self: error; emit_doctype; to Data),
                _       => go!(self: error; to BogusDoctype),
                }
            },
            //§ doctype_public_identifier_double_quoted_state doctype_system_identifier_double_quoted_state
            XmlState::DoctypeIdentifierDoubleQuoted(kind) => loop { match get_char!(self) {
                '"'     => go!(self: to AfterDoctypeIdentifier kind),
                '>'     => go!(self: error; emit_doctype; to Data),
                c       => go!(self: push_doctype_id kind c),
                }
            },
            //§ doctype_public_identifier_single_quoted_state doctype_system_identifier_single_quoted_state
            XmlState::DoctypeIdentifierSingleQuoted(kind) => loop { match get_char!(self) {
                '\''    => go!(self: to AfterDoctypeIdentifier kind),
                '>'     => go!(self: error; emit_doctype; to Data),
                c       => go!(self: push_doctype_id kind c),
                }
            },
            //§ doctype_public_identifier_single_quoted_state
            XmlState::AfterDoctypeIdentifier(Public) => loop { match get_char!(self) {
                '\t' | '\n' | '\x0C'
                | ' '   => go!(self: to BetweenDoctypePublicAndSystemIdentifiers),
                '\''    => go!(self: error; clear_doctype_id System; to DoctypeIdentifierSingleQuoted(System)),
                '"'     => go!(self: error; clear_doctype_id System; to DoctypeIdentifierDoubleQuoted(System)),
                '>'     => go!(self: emit_doctype; to Data),
                _       => go!(self: error; to BogusDoctype),
                }
            },
            //§ doctype_system_identifier_single_quoted_state
            XmlState::AfterDoctypeIdentifier(System) => loop { match get_char!(self) {
                '\t' | '\n' | '\x0C'
                | ' '   => (),
                '>'     => go!(self: emit_doctype; to Data),
                _       => go!(self: error; to BogusDoctype),
                }
            },
            //§ between_doctype_public_and_system_identifier_state
            XmlState::BetweenDoctypePublicAndSystemIdentifiers => loop { match get_char!(self) {
                '\t' | '\n' | '\x0C'
                | ' '   => (),
                '>'     => go!(self: emit_doctype; to Data),
                '\''    => go!(self: to DoctypeIdentifierSingleQuoted(System)),
                '"'     => go!(self: to DoctypeIdentifierDoubleQuoted(System)),
                _       => go!(self: error; to BogusDoctype),
                }
            },
            //§ bogus_doctype_state
            XmlState::BogusDoctype => loop { match get_char!(self) {
                '>'     => go!(self: emit_doctype; to Data),
                _       => (),
                }
            }
        }
    }

    /// Indicate that we have reached the end of the input.
    pub fn end(&mut self) {
        // Handle EOF in the char ref sub-tokenizer, if there is one.
        // Do this first because it might un-consume stuff.
        match self.char_ref_tokenizer.take() {
            None => (),
            Some(mut tok) => {
                tok.end_of_file(self);
                self.process_char_ref(tok.get_result());
            }
        }

        // Process all remaining buffered input.
        // If we're waiting for lookahead, we're not gonna get it.
        self.at_eof = true;
        self.run();

        while self.eof_step() {
            // loop
        }

        if self.opts.profile {
            self.dump_profile();
        }
    }
    #[cfg(for_c)]
    fn dump_profile(&self) {
        unreachable!();
    }

    #[cfg(not(for_c))]
    fn dump_profile(&self) {
        let mut results: Vec<(states::XmlState, u64)>
            = self.state_profile.iter().map(|(s, t)| (*s, *t)).collect();
        results.sort_by(|&(_, x), &(_, y)| y.cmp(&x));

        let total: u64 = results.iter().map(|&(_, t)| t).fold(0, ::std::ops::Add::add);
        println!("\nTokenizer profile, in nanoseconds");
        println!("\n{:12}         total in token sink", self.time_in_sink);
        println!("\n{:12}         total in tokenizer", total);

        for (k, v) in results.into_iter() {
            let pct = 100.0 * (v as f64) / (total as f64);
            println!("{:12}  {:4.1}%  {:?}", v, pct, k);
        }
    }


    fn eof_step(&mut self) -> bool {
        debug!("processing EOF in state {:?}", self.state);
        match self.state {
            XmlState::Data
                => go!(self: eof),
            XmlState::TagState
                => go!(self: error_eof; emit '<'; to Data),
            XmlState::EndTagState
                => go!(self: error_eof; emit '<'; emit '/'; to Data),
            XmlState::TagEmpty
                => go!(self: error_eof; to TagAttrNameBefore),
            XmlState::Cdata
            | XmlState::CdataBracket | XmlState::CdataEnd
                => go!(self: error_eof; to Data),
            XmlState::Pi
                => go!(self: error_eof; to BogusComment),
            XmlState::PiTargetAfter | XmlState::PiAfter
                => go!(self: reconsume PiData),
            XmlState::MarkupDecl
                => go!(self: error_eof; to BogusComment),
            XmlState::Comment | XmlState::CommentDash
            | XmlState::CommentEnd
                => go!(self: error_eof; emit_comment;to Data),
            XmlState::TagName | XmlState::TagAttrNameBefore
            | XmlState::EndTagName | XmlState::TagAttrNameAfter
            | XmlState::EndTagNameAfter | XmlState::TagAttrValueBefore
            | XmlState::TagAttrValue(_)
                => go!(self: error_eof; emit_tag Data),
            XmlState::PiData | XmlState::PiTarget
                => go!(self: error_eof; emit_pi Data),
            XmlState::TagAttrName
                => go!(self: error_eof; emit_start_tag Data),
            XmlState::BeforeDoctypeName
            | XmlState::Doctype | XmlState::DoctypeName
            | XmlState::AfterDoctypeName | XmlState::AfterDoctypeKeyword(_)
            | XmlState::BeforeDoctypeIdentifier(_) | XmlState::AfterDoctypeIdentifier(_)
            | XmlState::DoctypeIdentifierSingleQuoted(_) | XmlState::DoctypeIdentifierDoubleQuoted(_)
            | XmlState::BetweenDoctypePublicAndSystemIdentifiers
                => go!(self: error_eof; emit_doctype; to Data),
            XmlState::BogusDoctype
                => go!(self: emit_doctype; to Data),
            XmlState::BogusComment
                => go!(self: emit_comment; to Data),
        }
    }


    fn process_char_ref(&mut self, char_ref: CharRef) {
        let CharRef { mut chars, mut num_chars } = char_ref;

        if num_chars == 0 {
            chars[0] = '&';
            num_chars = 1;
        }

        for i in 0 .. num_chars {
            let c = chars[i as usize];
            match self.state {
                states::Data | states::Cdata
                    => go!(self: emit c),

                states::TagAttrValue(_)
                    => go!(self: push_value c),

                _ => panic!("state {:?} should not be reachable in process_char_ref", self.state),
            }
        }
    }

    fn step_char_ref_tokenizer(&mut self) -> bool {
        let mut tok = self.char_ref_tokenizer.take().unwrap();
        let outcome = tok.step(self);

        let progress = match outcome {
            char_ref::Done => {
                self.process_char_ref(tok.get_result());
                return true;
            }

            char_ref::Stuck => false,
            char_ref::Progress => true,
        };

        self.char_ref_tokenizer = Some(tok);
        progress
    }

    fn finish_attribute(&mut self) {
        if self.current_attr_name.len() == 0 {
            return;
        }

        // Check for a duplicate attribute.
        // FIXME: the spec says we should error as soon as the name is finished.
        // FIXME: linear time search, do we care?
        let dup = {
            let name = &self.current_attr_name[..];
            self.current_tag_attrs.iter().any(|a| &*a.name.local == name)
        };

        if dup {
            self.emit_error(Borrowed("Duplicate attribute"));
            self.current_attr_name.clear();
            self.current_attr_value.clear();
        } else {
            let qname = process_qname(replace(&mut self.current_attr_name, StrTendril::new()));
            let attr = Attribute {
                name: qname.clone(),
                value: replace(&mut self.current_attr_value, StrTendril::new()),
            };

            if &qname.local == &Atom::from("xmlns") ||
                &qname.prefix == &Atom::from("xmlns") {

                self.current_tag_attrs.insert(0, attr);
            } else {
                self.current_tag_attrs.push(attr);
            }
        }
    }

    fn create_attribute(&mut self, c: char) {
        self.finish_attribute();

        self.current_attr_name.push_char(c);
    }

}


#[cfg(test)]
mod test {

    use tendril::SliceExt;
    use string_cache::{Atom};
    use super::{process_qname};

    #[test]
    fn simple_namespace() {
        let qname = process_qname("prefix:local".to_tendril());
        assert_eq!(qname.prefix, Atom::from("prefix"));
        assert_eq!(qname.local, Atom::from("local"));

        let qname = process_qname("a:b".to_tendril());
        assert_eq!(qname.prefix, Atom::from("a"));
        assert_eq!(qname.local, Atom::from("b"));
    }

    #[test]
    fn wrong_namespaces() {
        let qname = process_qname(":local".to_tendril());
        assert_eq!(qname.prefix, Atom::from(""));
        assert_eq!(qname.local, Atom::from(":local"));

        let qname = process_qname("::local".to_tendril());
        assert_eq!(qname.prefix, Atom::from(""));
        assert_eq!(qname.local, Atom::from("::local"));

        let qname = process_qname("a::local".to_tendril());
        assert_eq!(qname.prefix, Atom::from(""));
        assert_eq!(qname.local, Atom::from("a::local"));

        let qname = process_qname("fake::".to_tendril());
        assert_eq!(qname.prefix, Atom::from(""));
        assert_eq!(qname.local, Atom::from("fake::"));

        let qname = process_qname(":::".to_tendril());
        assert_eq!(qname.prefix, Atom::from(""));
        assert_eq!(qname.local, Atom::from(":::"));

        let qname = process_qname(":a:b:".to_tendril());
        assert_eq!(qname.prefix, Atom::from(""));
        assert_eq!(qname.local, Atom::from(":a:b:"));
    }
}
