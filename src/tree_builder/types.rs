pub use self::XmlPhase::*;
pub use self::XmlProcessResult::*;
pub use self::XToken::*;

use tendril::StrTendril;
use tokenizer::{XTag, XPi};

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum XmlPhase {
    StartPhase,
    MainPhase,
    EndPhase,
}

/// A subset/refinement of `tokenizer::XToken`.  Everything else is handled
/// specially at the beginning of `process_token`.
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum XToken {
    XTagToken(XTag),
    CommentXToken(StrTendril),
    CharacterXTokens(StrTendril),
    PIToken(XPi),
    NullCharacterXToken,
    EOFXToken,
}

pub enum XmlProcessResult {
    XDone,
    XReprocess(XmlPhase, XToken),
}
