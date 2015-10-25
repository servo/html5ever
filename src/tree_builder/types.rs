pub use self::XmlPhase::*;
pub use self::XmlProcessResult::*;
pub use self::Token::*;

use tendril::StrTendril;
use tokenizer::{Tag, Pi};

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum XmlPhase {
    StartPhase,
    MainPhase,
    EndPhase,
}

/// A subset/refinement of `tokenizer::XToken`.  Everything else is handled
/// specially at the beginning of `process_token`.
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Token {
    TagToken(Tag),
    CommentToken(StrTendril),
    CharacterTokens(StrTendril),
    PIToken(Pi),
    NullCharacterToken,
    EOFToken,
}

pub enum XmlProcessResult {
    Done,
    Reprocess(XmlPhase, Token),
}
