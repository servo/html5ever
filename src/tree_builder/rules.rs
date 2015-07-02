use std::borrow::Cow::Borrowed;
use tendril::StrTendril;
use tokenizer::{XTag, StartXTag, EndXTag, ShortXTag, EmptyXTag};
use tree_builder::types::*;
use tree_builder::interface::TreeSink;
use tree_builder::actions::XmlTreeBuilderActions;

fn any_not_whitespace(x: &StrTendril) -> bool {
    !x.bytes().all(|b| matches!(b, b'\t' | b'\r' | b'\n' | b'\x0C' | b' '))
}

pub trait XmlTreeBuilderStep {
    fn step(&mut self, mode: XmlPhase, token: XToken) -> XmlProcessResult;
}

#[doc(hidden)]
impl<Handle, Sink> XmlTreeBuilderStep
    for super::XmlTreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{

    fn step(&mut self, mode: XmlPhase, token: XToken) -> XmlProcessResult {
        self.debug_step(mode, &token);

        match mode {
            StartPhase => match token {
                XTagToken(XTag{kind: StartXTag, name, attrs}) => {
                    let tag = XTag {
                        kind: StartXTag,
                        name: name,
                        attrs: attrs
                    };
                    self.phase = MainPhase;
                    let handle = self.append_tag_to_doc(tag);
                    self.add_to_open_elems(handle)

                },
                XTagToken(XTag{kind: EmptyXTag, name, attrs}) => {
                    let tag = XTag {
                        kind: StartXTag,
                        name: name,
                        attrs: attrs
                    };
                    self.phase = EndPhase;
                    self.append_tag_to_doc(tag);
                    XDone
                },
                CommentXToken(comment) => {
                    self.append_comment_to_doc(comment)
                },
                PIToken(pi) => {
                    self.append_pi_to_doc(pi)
                },
                CharacterXTokens(ref chars)
                    if !any_not_whitespace(chars) => {
                        XDone
                },
                EOFXToken => {
                    self.sink.parse_error(Borrowed("Unexpected EOF in start phase"));
                    XReprocess(EndPhase, EOFXToken)
                },
                _ => {
                    self.sink.parse_error(Borrowed("Unexpected element in start phase"));
                    XDone
                },
            },
            MainPhase => match token {
                CharacterXTokens(chs) => {
                    self.append_text(chs)
                },
                XTagToken(XTag{kind: StartXTag, name, attrs}) => {
                    let tag = XTag {
                        kind: StartXTag,
                        name: name,
                        attrs: attrs
                    };

                    self.insert_tag(tag)
                },
                XTagToken(XTag{kind: EmptyXTag, name, attrs}) => {
                    let tag = XTag {
                        kind: StartXTag,
                        name: name,
                        attrs: attrs
                    };
                    self.append_tag(tag)
                },
                XTagToken(XTag{kind: EndXTag, name, attrs}) => {
                    let tag = XTag {
                        kind: StartXTag,
                        name: name,
                        attrs: attrs
                    };
                    println!("Enter EndXTag in MainPhase");
                    let retval = self.close_tag(tag);
                    if self.no_open_elems() {
                        println!("No open elems, switch to EndPhase");
                        self.phase = EndPhase;
                    }
                    retval
                },
                XTagToken(XTag{kind: ShortXTag, ..}) => {
                    self.pop();
                    if self.no_open_elems() {
                        self.phase = EndPhase;
                    }
                    XDone
                },
                CommentXToken(comment) => {
                    self.append_comment_to_tag(comment)
                },
                PIToken(pi) => {
                    self.append_pi_to_tag(pi)
                },
                EOFXToken | NullCharacterXToken=> {
                    XReprocess(EndPhase, EOFXToken)
                }
            },
            EndPhase => match token {
                CommentXToken(comment) => {
                    self.append_comment_to_doc(comment)
                },
                PIToken(pi) => {
                    self.append_pi_to_doc(pi)
                },
                CharacterXTokens(ref chars)
                    if !any_not_whitespace(chars) => {
                        XDone
                },
                EOFXToken => {
                    self.stop_parsing()
                }
                _ => {
                    self.sink.parse_error(Borrowed("Unexpected element in end phase"));
                    XDone
                }
            },

        }
    }
}
