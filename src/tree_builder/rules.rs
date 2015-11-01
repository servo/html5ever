use std::borrow::Cow::Borrowed;
use tendril::StrTendril;
use tokenizer::{Tag, StartTag, EndTag, ShortTag, EmptyTag};
use tree_builder::types::*;
use tree_builder::interface::TreeSink;
use tree_builder::actions::XmlTreeBuilderActions;

fn any_not_whitespace(x: &StrTendril) -> bool {
    !x.bytes().all(|b| matches!(b, b'\t' | b'\r' | b'\n' | b'\x0C' | b' '))
}

pub trait XmlTreeBuilderStep {
    fn step(&mut self, mode: XmlPhase, token: Token) -> XmlProcessResult;
}

#[doc(hidden)]
impl<Handle, Sink> XmlTreeBuilderStep
    for super::XmlTreeBuilder<Handle, Sink>
    where Handle: Clone,
          Sink: TreeSink<Handle=Handle>,
{

    fn step(&mut self, mode: XmlPhase, token: Token) -> XmlProcessResult {
        self.debug_step(mode, &token);

        match mode {
            StartPhase => match token {
                TagToken(Tag{kind: StartTag, name, attrs}) => {
                    let tag = self.process_namespaces(Tag {
                        kind: StartTag,
                        name: name,
                        attrs: attrs,
                    });
                    self.phase = MainPhase;
                    let handle = self.append_tag_to_doc(tag);
                    self.add_to_open_elems(handle)

                },
                TagToken(Tag{kind: EmptyTag, name, attrs}) => {
                    let tag = self.process_namespaces(Tag {
                        kind: StartTag,
                        name: name,
                        attrs: attrs,
                    });
                    self.phase = EndPhase;
                    self.append_tag_to_doc(tag);
                    Done
                },
                CommentToken(comment) => {
                    self.append_comment_to_doc(comment)
                },
                PIToken(pi) => {
                    self.append_pi_to_doc(pi)
                },
                CharacterTokens(ref chars)
                    if !any_not_whitespace(chars) => {
                        Done
                },
                EOFToken => {
                    self.sink.parse_error(Borrowed("Unexpected EOF in start phase"));
                    Reprocess(EndPhase, EOFToken)
                },
                _ => {
                    self.sink.parse_error(Borrowed("Unexpected element in start phase"));
                    Done
                },
            },
            MainPhase => match token {
                CharacterTokens(chs) => {
                    self.append_text(chs)
                },
                TagToken(Tag{kind: StartTag, name, attrs}) => {
                    let tag =  self.process_namespaces(Tag {
                        kind: StartTag,
                        name: name,
                        attrs: attrs,
                    });

                    self.insert_tag(tag)
                },
                TagToken(Tag{kind: EmptyTag, name, attrs}) => {
                    let tag =  self.process_namespaces(Tag {
                        kind: EmptyTag,
                        name: name,
                        attrs: attrs,
                    });
                    self.append_tag(tag)
                },
                TagToken(Tag{kind: EndTag, name, attrs}) => {
                    let tag =  self.process_namespaces(Tag {
                        kind: EndTag,
                        name: name,
                        attrs: attrs,
                    });
                    println!("Enter EndTag in MainPhase");
                    let retval = self.close_tag(tag);
                    if self.no_open_elems() {
                        println!("No open elems, switch to EndPhase");
                        self.phase = EndPhase;
                    }
                    retval
                },
                TagToken(Tag{kind: ShortTag, ..}) => {
                    self.pop();
                    if self.no_open_elems() {
                        self.phase = EndPhase;
                    }
                    Done
                },
                CommentToken(comment) => {
                    self.append_comment_to_tag(comment)
                },
                PIToken(pi) => {
                    self.append_pi_to_tag(pi)
                },
                EOFToken | NullCharacterToken=> {
                    Reprocess(EndPhase, EOFToken)
                }
            },
            EndPhase => match token {
                CommentToken(comment) => {
                    self.append_comment_to_doc(comment)
                },
                PIToken(pi) => {
                    self.append_pi_to_doc(pi)
                },
                CharacterTokens(ref chars)
                    if !any_not_whitespace(chars) => {
                        Done
                },
                EOFToken => {
                    self.stop_parsing()
                }
                _ => {
                    self.sink.parse_error(Borrowed("Unexpected element in end phase"));
                    Done
                }
            },

        }
    }
}
