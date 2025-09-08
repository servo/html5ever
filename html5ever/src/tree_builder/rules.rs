// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// The tree builder rules, as a single, enormous nested match expression.

use crate::interface::Quirks;
use crate::tokenizer::states::{Rawtext, Rcdata, ScriptData};
use crate::tokenizer::TagKind::{EndTag, StartTag};
use crate::tree_builder::tag_sets::*;
use crate::tree_builder::types::*;
use crate::tree_builder::{
    create_element, html_elem, ElemName, NodeOrText::AppendNode, StrTendril, Tag, TreeBuilder,
    TreeSink,
};
use crate::QualName;
use markup5ever::{expanded_name, local_name, ns};
use std::borrow::Cow::Borrowed;

use crate::tendril::SliceExt;
use match_token::match_token;

fn any_not_whitespace(x: &StrTendril) -> bool {
    // FIXME: this might be much faster as a byte scan
    x.chars().any(|c| !c.is_ascii_whitespace())
}

fn current_node<Handle>(open_elems: &[Handle]) -> &Handle {
    open_elems.last().expect("no current element")
}

#[rustfmt::skip]
macro_rules! tag {
    // Any start tag
    (<>) => {
        crate::tokenizer::Tag { kind: crate::tokenizer::StartTag, .. }
    };
    (<>|$($tail:tt)*) => {
        tag!(<>) | tag!($($tail)*)
    };

    // Any end tag
    (</>) => {
        crate::tokenizer::Tag { kind: crate::tokenizer::EndTag, .. }
    };
    (</>|$($tail:tt)*) => {
        tag!(</>) | tag!($($tail)*)
    };

    // Named start tag
    (<$tag:tt>) => {
        crate::tokenizer::Tag { kind: crate::tokenizer::StartTag, name: local_name!($tag), .. }
    };
    (<$tag:tt>|$($tail:tt)*) => {
        tag!(<$tag>) | tag!($($tail)*)
    };

    // Named end tag
    (</$tag:tt>) => {
        crate::tokenizer::Tag { kind: crate::tokenizer::EndTag, name: local_name!($tag), .. }
    };
    (</$tag:tt>|$($tail:tt)*) => {
        tag!(</$tag>) | tag!($($tail)*)
    };
}

#[doc(hidden)]
impl<Handle, Sink> TreeBuilder<Handle, Sink>
where
    Handle: Clone,
    Sink: TreeSink<Handle = Handle>,
{
    /// Process an HTML content token
    ///
    /// <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhtml>
    pub(crate) fn step(&self, mode: InsertionMode, token: Token) -> ProcessResult<Handle> {
        self.debug_step(mode, &token);

        match mode {
            // § the-initial-insertion-mode
            // <https://html.spec.whatwg.org/multipage/parsing.html#the-initial-insertion-mode>
            InsertionMode::Initial => match token {
                Token::Characters(SplitStatus::NotSplit, text) => {
                    ProcessResult::SplitWhitespace(text)
                },
                Token::Characters(SplitStatus::Whitespace, _) => ProcessResult::Done,
                Token::Comment(text) => self.append_comment_to_doc(text),
                token => {
                    if !self.opts.iframe_srcdoc {
                        self.unexpected(&token);
                        self.set_quirks_mode(Quirks);
                    }
                    ProcessResult::Reprocess(InsertionMode::BeforeHtml, token)
                },
            },

            // § the-before-html-insertion-mode
            // <https://html.spec.whatwg.org/multipage/parsing.html#the-before-html-insertion-mode>
            InsertionMode::BeforeHtml => {
                let anything_else = |token: Token| {
                    self.create_root(vec![]);
                    ProcessResult::Reprocess(InsertionMode::BeforeHead, token)
                };

                match token {
                    Token::Comment(text) => self.append_comment_to_doc(text),

                    Token::Characters(SplitStatus::NotSplit, text) => {
                        ProcessResult::SplitWhitespace(text)
                    },

                    Token::Characters(SplitStatus::Whitespace, _) => ProcessResult::Done,
                    Token::Tag(tag @ tag!(<html>)) => {
                        self.create_root(tag.attrs);
                        self.mode.set(InsertionMode::BeforeHead);
                        ProcessResult::Done
                    },

                    Token::Tag(tag!(</head> | </body> | </html> | </br>)) => anything_else(token),
                    Token::Tag(tag @ tag!(</>)) => self.unexpected(&tag),

                    token => anything_else(token),
                }
            },

            // § the-before-head-insertion-mode
            // <https://html.spec.whatwg.org/multipage/parsing.html#the-before-head-insertion-mode>
            InsertionMode::BeforeHead => {
                let anything_else = |token: Token| {
                    *self.head_elem.borrow_mut() = Some(self.insert_phantom(local_name!("head")));
                    ProcessResult::Reprocess(InsertionMode::InHead, token)
                };
                match token {
                    Token::Characters(SplitStatus::NotSplit, text) => {
                        ProcessResult::SplitWhitespace(text)
                    },
                    Token::Characters(SplitStatus::Whitespace, _) => ProcessResult::Done,
                    Token::Comment(text) => self.append_comment(text),

                    Token::Tag(tag!(<html>)) => self.step(InsertionMode::InBody, token),

                    Token::Tag(tag @ tag!(<head>)) => {
                        *self.head_elem.borrow_mut() = Some(self.insert_element_for(tag));
                        self.mode.set(InsertionMode::InHead);
                        ProcessResult::Done
                    },

                    Token::Tag(tag!(</head> | </body> | </html> | </br>)) => anything_else(token),

                    Token::Tag(tag @ tag!(</>)) => self.unexpected(&tag),

                    token => anything_else(token),
                }
            },

            // § parsing-main-inhead
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inhead>
            InsertionMode::InHead => {
                let anything_else = |token: Token| {
                    self.pop();
                    ProcessResult::Reprocess(InsertionMode::AfterHead, token)
                };

                match token {
                    Token::Characters(SplitStatus::NotSplit, text) => {
                        ProcessResult::SplitWhitespace(text)
                    },
                    Token::Characters(SplitStatus::Whitespace, text) => self.append_text(text),
                    Token::Comment(text) => self.append_comment(text),

                    Token::Tag(tag!(<html>)) => self.step(InsertionMode::InBody, token),

                    Token::Tag(tag @ tag!(<base> | <basefont> | <bgsound> | <link> | <meta>)) => {
                        // FIXME: handle <meta charset=...> and <meta http-equiv="Content-Type">
                        self.insert_and_pop_element_for(tag);
                        ProcessResult::DoneAckSelfClosing
                    },

                    Token::Tag(tag @ tag!(<title>)) => self.parse_raw_data(tag, Rcdata),

                    Token::Tag(tag @ tag!(<noframes> | <style> | <noscript>)) => {
                        if (!self.opts.scripting_enabled) && (tag.name == local_name!("noscript")) {
                            self.insert_element_for(tag);
                            self.mode.set(InsertionMode::InHeadNoscript);
                            ProcessResult::Done
                        } else {
                            self.parse_raw_data(tag, Rawtext)
                        }
                    },

                    Token::Tag(tag @ tag!(<script>)) => {
                        let elem = create_element(
                            &self.sink,
                            QualName::new(None, ns!(html), local_name!("script")),
                            tag.attrs,
                        );
                        if self.is_fragment() {
                            self.sink.mark_script_already_started(&elem);
                        }
                        self.insert_appropriately(AppendNode(elem.clone()), None);
                        self.open_elems.borrow_mut().push(elem);
                        self.to_raw_text_mode(ScriptData)
                    },

                    Token::Tag(tag!(</head>)) => {
                        self.pop();
                        self.mode.set(InsertionMode::AfterHead);
                        ProcessResult::Done
                    },

                    Token::Tag(tag!(</body> | </html> | </br>)) => anything_else(token),

                    Token::Tag(tag @ tag!(<template>)) => {
                        self.active_formatting
                            .borrow_mut()
                            .push(FormatEntry::Marker);
                        self.frameset_ok.set(false);
                        self.mode.set(InsertionMode::InTemplate);
                        self.template_modes
                            .borrow_mut()
                            .push(InsertionMode::InTemplate);

                        if (self.should_attach_declarative_shadow(&tag)) {
                            // Attach shadow path

                            // Step 1. Let declarative shadow host element be adjusted current node.
                            let mut shadow_host = self.open_elems.borrow().last().unwrap().clone();
                            if self.is_fragment() && self.open_elems.borrow().len() == 1 {
                                shadow_host = self.context_elem.borrow().clone().unwrap();
                            }

                            // Step 2. Let template be the result of insert a foreign element for template start tag, with HTML namespace and true.
                            let template =
                                self.insert_foreign_element(tag.clone(), ns!(html), true);

                            // Step 3 - 8.
                            // Attach a shadow root with declarative shadow host element, mode, clonable, serializable, delegatesFocus, and "named".
                            let succeeded =
                                self.attach_declarative_shadow(&tag, &shadow_host, &template);
                            if !succeeded {
                                // Step 8.1.1. Insert an element at the adjusted insertion location with template.
                                // Pop the current template element created in step 2 first.
                                self.pop();
                                self.insert_element_for(tag);
                            }
                        } else {
                            self.insert_element_for(tag);
                        }

                        ProcessResult::Done
                    },

                    Token::Tag(tag @ tag!(</template>)) => {
                        if !self.in_html_elem_named(local_name!("template")) {
                            self.unexpected(&tag);
                        } else {
                            self.generate_implied_end_tags(thorough_implied_end);
                            self.expect_to_close(local_name!("template"));
                            self.clear_active_formatting_to_marker();
                            self.template_modes.borrow_mut().pop();
                            self.mode.set(self.reset_insertion_mode());
                        }
                        ProcessResult::Done
                    },

                    Token::Tag(tag!(<head> | </>)) => self.unexpected(&token),

                    token => anything_else(token),
                }
            },

            // § parsing-main-inheadnoscript
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inheadnoscript>
            InsertionMode::InHeadNoscript => {
                let anything_else = |token: Token| {
                    self.unexpected(&token);
                    self.pop();
                    ProcessResult::Reprocess(InsertionMode::InHead, token)
                };
                match token {
                    Token::Tag(tag!(<html>)) => self.step(InsertionMode::InBody, token),

                    Token::Tag(tag!(</noscript>)) => {
                        self.pop();
                        self.mode.set(InsertionMode::InHead);
                        ProcessResult::Done
                    },

                    Token::Characters(SplitStatus::NotSplit, text) => {
                        ProcessResult::SplitWhitespace(text)
                    },
                    Token::Characters(SplitStatus::Whitespace, _) => {
                        self.step(InsertionMode::InHead, token)
                    },

                    Token::Comment(_) => self.step(InsertionMode::InHead, token),

                    Token::Tag(
                        tag!(<basefont> | <bgsound> | <link> | <meta> | <noframes> | <style>),
                    ) => self.step(InsertionMode::InHead, token),

                    Token::Tag(tag!(</br>)) => anything_else(token),

                    Token::Tag(tag!(<head> | <noscript> | </>)) => self.unexpected(&token),

                    token => anything_else(token),
                }
            },

            // § the-after-head-insertion-mode
            // <https://html.spec.whatwg.org/multipage/parsing.html#the-after-head-insertion-mode>
            InsertionMode::AfterHead => {
                let anything_else = |token: Token| {
                    self.insert_phantom(local_name!("body"));
                    ProcessResult::Reprocess(InsertionMode::InBody, token)
                };

                match token {
                    Token::Characters(SplitStatus::NotSplit, text) => {
                        ProcessResult::SplitWhitespace(text)
                    },
                    Token::Characters(SplitStatus::Whitespace, text) => self.append_text(text),
                    Token::Comment(text) => self.append_comment(text),

                    Token::Tag(tag!(<html>)) => self.step(InsertionMode::InBody, token),

                    Token::Tag(tag @ tag!(<body>)) => {
                        self.insert_element_for(tag);
                        self.frameset_ok.set(false);
                        self.mode.set(InsertionMode::InBody);
                        ProcessResult::Done
                    },

                    Token::Tag(tag @ tag!(<frameset>)) => {
                        self.insert_element_for(tag);
                        self.mode.set(InsertionMode::InFrameset);
                        ProcessResult::Done
                    },

                    Token::Tag(
                        tag!(<base> | <basefont> | <bgsound> | <link> | <meta> |
                                <noframes> | <script> | <style> | <template> | <title>
                        ),
                    ) => {
                        self.unexpected(&token);
                        let head = self
                            .head_elem
                            .borrow()
                            .as_ref()
                            .expect("no head element")
                            .clone();
                        self.push(&head);
                        let result = self.step(InsertionMode::InHead, token);
                        self.remove_from_stack(&head);
                        result
                    },

                    Token::Tag(tag!(</template>)) => self.step(InsertionMode::InHead, token),

                    Token::Tag(tag!(</body> | </html> | </br>)) => anything_else(token),

                    Token::Tag(tag!(<head> | </>)) => self.unexpected(&token),

                    token => anything_else(token),
                }
            },

            // § parsing-main-inbody
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody>
            InsertionMode::InBody => match token {
                Token::NullCharacter => self.unexpected(&token),

                Token::Characters(_, text) => {
                    self.reconstruct_active_formatting_elements();
                    if any_not_whitespace(&text) {
                        self.frameset_ok.set(false);
                    }
                    self.append_text(text)
                },

                Token::Comment(text) => self.append_comment(text),

                Token::Tag(tag @ tag!(<html>)) => {
                    self.unexpected(&tag);
                    if !self.in_html_elem_named(local_name!("template")) {
                        let open_elems = self.open_elems.borrow();
                        let top = html_elem(&open_elems);
                        self.sink.add_attrs_if_missing(top, tag.attrs);
                    }
                    ProcessResult::Done
                },

                Token::Tag(
                    tag!(<base> | <basefont> | <bgsound> | <link> | <meta> | <noframes>
                            | <script> | <style> | <template> | <title> | </template>),
                ) => self.step(InsertionMode::InHead, token),

                Token::Tag(tag @ tag!(<body>)) => {
                    self.unexpected(&tag);
                    let body_elem = self.body_elem().as_deref().cloned();
                    match body_elem {
                        Some(ref node)
                            if self.open_elems.borrow().len() != 1
                                && !self.in_html_elem_named(local_name!("template")) =>
                        {
                            self.frameset_ok.set(false);
                            self.sink.add_attrs_if_missing(node, tag.attrs)
                        },
                        _ => {},
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<frameset>)) => {
                    self.unexpected(&tag);
                    if !self.frameset_ok.get() {
                        return ProcessResult::Done;
                    }

                    let Some(body) = self.body_elem().map(|b| b.clone()) else {
                        return ProcessResult::Done;
                    };
                    self.sink.remove_from_parent(&body);

                    // FIXME: can we get here in the fragment case?
                    // What to do with the first element then?
                    self.open_elems.borrow_mut().truncate(1);
                    self.insert_element_for(tag);
                    self.mode.set(InsertionMode::InFrameset);
                    ProcessResult::Done
                },

                Token::Eof => {
                    if !self.template_modes.borrow().is_empty() {
                        self.step(InsertionMode::InTemplate, token)
                    } else {
                        self.check_body_end();
                        self.stop_parsing()
                    }
                },

                Token::Tag(tag!(</body>)) => {
                    if self.in_scope_named(default_scope, local_name!("body")) {
                        self.check_body_end();
                        self.mode.set(InsertionMode::AfterBody);
                    } else {
                        self.sink
                            .parse_error(Borrowed("</body> with no <body> in scope"));
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag!(</html>)) => {
                    if self.in_scope_named(default_scope, local_name!("body")) {
                        self.check_body_end();
                        ProcessResult::Reprocess(InsertionMode::AfterBody, token)
                    } else {
                        self.sink
                            .parse_error(Borrowed("</html> with no <body> in scope"));
                        ProcessResult::Done
                    }
                },

                Token::Tag(
                    tag @
                    tag!(<address> | <article> | <aside> | <blockquote> | <center> | <details> | <dialog> |
                          <dir> | <div> | <dl> | <fieldset> | <figcaption> | <figure> | <footer> | <header> |
                          <hgroup> | <main> | <nav> | <ol> | <p> | <search> | <section> | <summary> | <ul>),
                ) => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<menu>)) => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<h1> | <h2> | <h3> | <h4> | <h5> | <h6>)) => {
                    self.close_p_element_in_button_scope();
                    if self.current_node_in(heading_tag) {
                        self.sink.parse_error(Borrowed("nested heading tags"));
                        self.pop();
                    }
                    self.insert_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<pre> | <listing>)) => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    self.ignore_lf.set(true);
                    self.frameset_ok.set(false);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<form>)) => {
                    if self.form_elem.borrow().is_some()
                        && !self.in_html_elem_named(local_name!("template"))
                    {
                        self.sink.parse_error(Borrowed("nested forms"));
                    } else {
                        self.close_p_element_in_button_scope();
                        let elem = self.insert_element_for(tag);
                        if !self.in_html_elem_named(local_name!("template")) {
                            *self.form_elem.borrow_mut() = Some(elem);
                        }
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<li> | <dd> | <dt>)) => {
                    declare_tag_set!(close_list = "li");
                    declare_tag_set!(close_defn = "dd" "dt");
                    declare_tag_set!(extra_special = [special_tag] - "address" "div" "p");
                    let list = match tag.name {
                        local_name!("li") => true,
                        local_name!("dd") | local_name!("dt") => false,
                        _ => unreachable!(),
                    };

                    self.frameset_ok.set(false);

                    let mut to_close = None;
                    for node in self.open_elems.borrow().iter().rev() {
                        let elem_name = self.sink.elem_name(node);
                        let name = elem_name.expanded();
                        let can_close = if list {
                            close_list(name)
                        } else {
                            close_defn(name)
                        };
                        if can_close {
                            to_close = Some(name.local.clone());
                            break;
                        }
                        if extra_special(name) {
                            break;
                        }
                    }

                    if let Some(name) = to_close {
                        self.generate_implied_end_except(name.clone());
                        self.expect_to_close(name);
                    }

                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<plaintext>)) => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    ProcessResult::ToPlaintext
                },

                Token::Tag(tag @ tag!(<button>)) => {
                    if self.in_scope_named(default_scope, local_name!("button")) {
                        self.sink.parse_error(Borrowed("nested buttons"));
                        self.generate_implied_end_tags(cursory_implied_end);
                        self.pop_until_named(local_name!("button"));
                    }
                    self.reconstruct_active_formatting_elements();
                    self.insert_element_for(tag);
                    self.frameset_ok.set(false);
                    ProcessResult::Done
                },

                Token::Tag(
                    tag @
                    tag!(</address> | </article> | </aside> | </blockquote> | </button> | </center> |
                              </details> | </dialog> | </dir> | </div> | </dl> | </fieldset> | </figcaption> |
                              </figure> | </footer> | </header> | </hgroup> | </listing> | </main> | </menu> |
                              </nav> | </ol> | </pre> | </search> | </section> | </summary> | </ul>),
                ) => {
                    if !self.in_scope_named(default_scope, tag.name.clone()) {
                        self.unexpected(&tag);
                    } else {
                        self.generate_implied_end_tags(cursory_implied_end);
                        self.expect_to_close(tag.name);
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag!(</form>)) => {
                    if !self.in_html_elem_named(local_name!("template")) {
                        let Some(node) = self.form_elem.take() else {
                            self.sink
                                .parse_error(Borrowed("Null form element pointer on </form>"));
                            return ProcessResult::Done;
                        };
                        if !self.in_scope(default_scope, |n| self.sink.same_node(&node, &n)) {
                            self.sink
                                .parse_error(Borrowed("Form element not in scope on </form>"));
                            return ProcessResult::Done;
                        }
                        self.generate_implied_end_tags(cursory_implied_end);
                        let current = self.current_node().clone();
                        self.remove_from_stack(&node);
                        if !self.sink.same_node(&current, &node) {
                            self.sink
                                .parse_error(Borrowed("Bad open element on </form>"));
                        }
                    } else {
                        if !self.in_scope_named(default_scope, local_name!("form")) {
                            self.sink
                                .parse_error(Borrowed("Form element not in scope on </form>"));
                            return ProcessResult::Done;
                        }
                        self.generate_implied_end_tags(cursory_implied_end);
                        if !self.current_node_named(local_name!("form")) {
                            self.sink
                                .parse_error(Borrowed("Bad open element on </form>"));
                        }
                        self.pop_until_named(local_name!("form"));
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag!(</p>)) => {
                    if !self.in_scope_named(button_scope, local_name!("p")) {
                        self.sink.parse_error(Borrowed("No <p> tag to close"));
                        self.insert_phantom(local_name!("p"));
                    }
                    self.close_p_element();
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(</li> | </dd> | </dt>)) => {
                    let in_scope = if tag.name == local_name!("li") {
                        self.in_scope_named(list_item_scope, tag.name.clone())
                    } else {
                        self.in_scope_named(default_scope, tag.name.clone())
                    };
                    if in_scope {
                        self.generate_implied_end_except(tag.name.clone());
                        self.expect_to_close(tag.name);
                    } else {
                        self.sink.parse_error(Borrowed("No matching tag to close"));
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(</h1> | </h2> | </h3> | </h4> | </h5> | </h6>)) => {
                    if self.in_scope(default_scope, |n| self.elem_in(&n, heading_tag)) {
                        self.generate_implied_end_tags(cursory_implied_end);
                        if !self.current_node_named(tag.name) {
                            self.sink.parse_error(Borrowed("Closing wrong heading tag"));
                        }
                        self.pop_until(heading_tag);
                    } else {
                        self.sink.parse_error(Borrowed("No heading tag to close"));
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<a>)) => {
                    self.handle_misnested_a_tags(&tag);
                    self.reconstruct_active_formatting_elements();
                    self.create_formatting_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(
                    tag @
                    tag!(<b> | <big> | <code> | <em> | <font> | <i> | <s> | <small> | <strike> | <strong> | <tt> | <u>),
                ) => {
                    self.reconstruct_active_formatting_elements();
                    self.create_formatting_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<nobr>)) => {
                    self.reconstruct_active_formatting_elements();
                    if self.in_scope_named(default_scope, local_name!("nobr")) {
                        self.sink.parse_error(Borrowed("Nested <nobr>"));
                        self.adoption_agency(local_name!("nobr"));
                        self.reconstruct_active_formatting_elements();
                    }
                    self.create_formatting_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(
                    tag @ tag!(</a> | </b> | </big> | </code> | </em> | </font> | </i> | </nobr> |
                                </s> | </small> | </strike> | </strong> | </tt> | </u>),
                ) => {
                    self.adoption_agency(tag.name);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<applet> | <marquee> | <object>)) => {
                    self.reconstruct_active_formatting_elements();
                    self.insert_element_for(tag);
                    self.active_formatting
                        .borrow_mut()
                        .push(FormatEntry::Marker);
                    self.frameset_ok.set(false);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(</applet> | </marquee> | </object>)) => {
                    if !self.in_scope_named(default_scope, tag.name.clone()) {
                        self.unexpected(&tag);
                    } else {
                        self.generate_implied_end_tags(cursory_implied_end);
                        self.expect_to_close(tag.name);
                        self.clear_active_formatting_to_marker();
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<table>)) => {
                    if self.quirks_mode.get() != Quirks {
                        self.close_p_element_in_button_scope();
                    }
                    self.insert_element_for(tag);
                    self.frameset_ok.set(false);
                    self.mode.set(InsertionMode::InTable);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(</br>)) => {
                    self.unexpected(&tag);
                    self.step(
                        InsertionMode::InBody,
                        Token::Tag(Tag {
                            kind: StartTag,
                            attrs: vec![],
                            ..tag
                        }),
                    )
                },

                Token::Tag(
                    tag @ tag!(<area> | <br> | <embed> | <img> | <keygen> | <wbr> | <input>),
                ) => {
                    let keep_frameset_ok = match tag.name {
                        local_name!("input") => self.is_type_hidden(&tag),
                        _ => false,
                    };
                    self.reconstruct_active_formatting_elements();
                    self.insert_and_pop_element_for(tag);
                    if !keep_frameset_ok {
                        self.frameset_ok.set(false);
                    }
                    ProcessResult::DoneAckSelfClosing
                },

                Token::Tag(tag @ tag!(<param> | <source> | <track>)) => {
                    self.insert_and_pop_element_for(tag);
                    ProcessResult::DoneAckSelfClosing
                },

                Token::Tag(tag @ tag!(<hr>)) => {
                    self.close_p_element_in_button_scope();
                    self.insert_and_pop_element_for(tag);
                    self.frameset_ok.set(false);
                    ProcessResult::DoneAckSelfClosing
                },

                Token::Tag(tag @ tag!(<image>)) => {
                    self.unexpected(&tag);
                    self.step(
                        InsertionMode::InBody,
                        Token::Tag(Tag {
                            name: local_name!("img"),
                            ..tag
                        }),
                    )
                },

                Token::Tag(tag @ tag!(<textarea>)) => {
                    self.ignore_lf.set(true);
                    self.frameset_ok.set(false);
                    self.parse_raw_data(tag, Rcdata)
                },

                Token::Tag(tag @ tag!(<xmp>)) => {
                    self.close_p_element_in_button_scope();
                    self.reconstruct_active_formatting_elements();
                    self.frameset_ok.set(false);
                    self.parse_raw_data(tag, Rawtext)
                },

                Token::Tag(tag @ tag!(<iframe>)) => {
                    self.frameset_ok.set(false);
                    self.parse_raw_data(tag, Rawtext)
                },

                Token::Tag(tag @ tag!(<noembed>)) => self.parse_raw_data(tag, Rawtext),

                // <noscript> handled in wildcard case below
                Token::Tag(tag @ tag!(<select>)) => {
                    self.reconstruct_active_formatting_elements();
                    self.insert_element_for(tag);
                    self.frameset_ok.set(false);
                    // NB: mode == InBody but possibly self.mode != mode, if
                    // we're processing "as in the rules for InBody".
                    self.mode.set(match self.mode.get() {
                        InsertionMode::InTable
                        | InsertionMode::InCaption
                        | InsertionMode::InTableBody
                        | InsertionMode::InRow
                        | InsertionMode::InCell => InsertionMode::InSelectInTable,
                        _ => InsertionMode::InSelect,
                    });
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<optgroup> | <option>)) => {
                    if self.current_node_named(local_name!("option")) {
                        self.pop();
                    }
                    self.reconstruct_active_formatting_elements();
                    self.insert_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<rb> | <rtc>)) => {
                    if self.in_scope_named(default_scope, local_name!("ruby")) {
                        self.generate_implied_end_tags(cursory_implied_end);
                    }
                    if !self.current_node_named(local_name!("ruby")) {
                        self.unexpected(&tag);
                    }
                    self.insert_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<rp> | <rt>)) => {
                    if self.in_scope_named(default_scope, local_name!("ruby")) {
                        self.generate_implied_end_except(local_name!("rtc"));
                    }
                    if !self.current_node_named(local_name!("rtc"))
                        && !self.current_node_named(local_name!("ruby"))
                    {
                        self.unexpected(&tag);
                    }
                    self.insert_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<math>)) => self.enter_foreign(tag, ns!(mathml)),

                Token::Tag(tag @ tag!(<svg>)) => self.enter_foreign(tag, ns!(svg)),

                Token::Tag(
                    tag!(<caption> | <col> | <colgroup> | <frame> | <head> |
                                <tbody> | <td> | <tfoot> | <th> | <thead> | <tr>),
                ) => {
                    self.unexpected(&token);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<>)) => {
                    if self.opts.scripting_enabled && tag.name == local_name!("noscript") {
                        self.parse_raw_data(tag, Rawtext)
                    } else {
                        self.reconstruct_active_formatting_elements();
                        self.insert_element_for(tag);
                        ProcessResult::Done
                    }
                },

                Token::Tag(tag @ tag!(</>)) => {
                    self.process_end_tag_in_body(tag);
                    ProcessResult::Done
                },
            },

            // § parsing-main-incdata
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incdata>
            InsertionMode::Text => match token {
                Token::Characters(_, text) => self.append_text(text),

                Token::Eof => {
                    self.unexpected(&token);
                    if self.current_node_named(local_name!("script")) {
                        let open_elems = self.open_elems.borrow();
                        let current = current_node(&open_elems);
                        self.sink.mark_script_already_started(current);
                    }
                    self.pop();
                    ProcessResult::Reprocess(self.orig_mode.take().unwrap(), token)
                },

                Token::Tag(tag @ tag!(</>)) => {
                    let node = self.pop();
                    self.mode.set(self.orig_mode.take().unwrap());
                    if tag.name == local_name!("script") {
                        return ProcessResult::Script(node);
                    }
                    ProcessResult::Done
                },

                // The spec doesn't say what to do here.
                // Other tokens are impossible?
                _ => unreachable!("impossible case in Text mode"),
            },

            // § parsing-main-intable
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable>
            InsertionMode::InTable => match token {
                Token::NullCharacter | Token::Characters(..) => self.process_chars_in_table(token),

                Token::Comment(text) => self.append_comment(text),

                Token::Tag(tag @ tag!(<caption>)) => {
                    self.pop_until_current(table_scope);
                    self.active_formatting
                        .borrow_mut()
                        .push(FormatEntry::Marker);
                    self.insert_element_for(tag);
                    self.mode.set(InsertionMode::InCaption);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<colgroup>)) => {
                    self.pop_until_current(table_scope);
                    self.insert_element_for(tag);
                    self.mode.set(InsertionMode::InColumnGroup);
                    ProcessResult::Done
                },

                Token::Tag(tag!(<col>)) => {
                    self.pop_until_current(table_scope);
                    self.insert_phantom(local_name!("colgroup"));
                    ProcessResult::Reprocess(InsertionMode::InColumnGroup, token)
                },

                Token::Tag(tag @ tag!(<tbody> | <tfoot> | <thead>)) => {
                    self.pop_until_current(table_scope);
                    self.insert_element_for(tag);
                    self.mode.set(InsertionMode::InTableBody);
                    ProcessResult::Done
                },

                Token::Tag(tag!(<td> | <th> | <tr>)) => {
                    self.pop_until_current(table_scope);
                    self.insert_phantom(local_name!("tbody"));
                    ProcessResult::Reprocess(InsertionMode::InTableBody, token)
                },

                Token::Tag(tag!(<table>)) => {
                    self.unexpected(&token);
                    if self.in_scope_named(table_scope, local_name!("table")) {
                        self.pop_until_named(local_name!("table"));
                        ProcessResult::Reprocess(self.reset_insertion_mode(), token)
                    } else {
                        ProcessResult::Done
                    }
                },

                Token::Tag(tag!(</table>)) => {
                    if self.in_scope_named(table_scope, local_name!("table")) {
                        self.pop_until_named(local_name!("table"));
                        self.mode.set(self.reset_insertion_mode());
                    } else {
                        self.unexpected(&token);
                    }
                    ProcessResult::Done
                },

                Token::Tag(
                    tag!(</body> | </caption> | </col> | </colgroup> | </html> |
                        </tbody> | </td> | </tfoot> | </th> | </thead> | </tr>),
                ) => self.unexpected(&token),

                Token::Tag(tag!(<style> | <script> | <template> | </template>)) => {
                    self.step(InsertionMode::InHead, token)
                },

                Token::Tag(tag @ tag!(<input>)) => {
                    self.unexpected(&tag);
                    if self.is_type_hidden(&tag) {
                        self.insert_and_pop_element_for(tag);
                        ProcessResult::DoneAckSelfClosing
                    } else {
                        self.foster_parent_in_body(Token::Tag(tag))
                    }
                },

                Token::Tag(tag @ tag!(<form>)) => {
                    self.unexpected(&tag);
                    if !self.in_html_elem_named(local_name!("template"))
                        && self.form_elem.borrow().is_none()
                    {
                        *self.form_elem.borrow_mut() = Some(self.insert_and_pop_element_for(tag));
                    }
                    ProcessResult::Done
                },

                Token::Eof => self.step(InsertionMode::InBody, token),

                token => {
                    self.unexpected(&token);
                    self.foster_parent_in_body(token)
                },
            },

            // § parsing-main-intabletext
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intabletext>
            InsertionMode::InTableText => match token {
                Token::NullCharacter => self.unexpected(&token),

                Token::Characters(split, text) => {
                    self.pending_table_text.borrow_mut().push((split, text));
                    ProcessResult::Done
                },

                token => {
                    let pending = self.pending_table_text.take();
                    let contains_nonspace = pending.iter().any(|&(split, ref text)| match split {
                        SplitStatus::Whitespace => false,
                        SplitStatus::NotWhitespace => true,
                        SplitStatus::NotSplit => any_not_whitespace(text),
                    });

                    if contains_nonspace {
                        self.sink.parse_error(Borrowed("Non-space table text"));
                        for (split, text) in pending.into_iter() {
                            match self.foster_parent_in_body(Token::Characters(split, text)) {
                                ProcessResult::Done => (),
                                _ => panic!("not prepared to handle this!"),
                            }
                        }
                    } else {
                        for (_, text) in pending.into_iter() {
                            self.append_text(text);
                        }
                    }

                    ProcessResult::Reprocess(self.orig_mode.take().unwrap(), token)
                },
            },

            // § parsing-main-incaption
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incaption>
            InsertionMode::InCaption => match token {
                Token::Tag(
                    tag @ tag!(<caption> | <col> | <colgroup> | <tbody> | <td> | <tfoot> |
                                <th> | <thead> | <tr> | </table> | </caption>),
                ) => {
                    if self.in_scope_named(table_scope, local_name!("caption")) {
                        self.generate_implied_end_tags(cursory_implied_end);
                        self.expect_to_close(local_name!("caption"));
                        self.clear_active_formatting_to_marker();
                        match tag {
                            Tag {
                                kind: EndTag,
                                name: local_name!("caption"),
                                ..
                            } => {
                                self.mode.set(InsertionMode::InTable);
                                ProcessResult::Done
                            },
                            _ => ProcessResult::Reprocess(InsertionMode::InTable, Token::Tag(tag)),
                        }
                    } else {
                        self.unexpected(&tag);
                        ProcessResult::Done
                    }
                },

                Token::Tag(
                    tag!(</body> | </col> | </colgroup> | </html> | </tbody> |
                            </td> | </tfoot> | </th> | </thead> | </tr>),
                ) => self.unexpected(&token),

                token => self.step(InsertionMode::InBody, token),
            },

            // § parsing-main-incolgroup
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-incolgroup>
            InsertionMode::InColumnGroup => match token {
                Token::Characters(SplitStatus::NotSplit, text) => {
                    ProcessResult::SplitWhitespace(text)
                },
                Token::Characters(SplitStatus::Whitespace, text) => self.append_text(text),
                Token::Comment(text) => self.append_comment(text),

                Token::Tag(tag!(<html>)) => self.step(InsertionMode::InBody, token),

                Token::Tag(tag @ tag!(<col>)) => {
                    self.insert_and_pop_element_for(tag);
                    ProcessResult::DoneAckSelfClosing
                },

                Token::Tag(tag!(</colgroup>)) => {
                    if self.current_node_named(local_name!("colgroup")) {
                        self.pop();
                        self.mode.set(InsertionMode::InTable);
                    } else {
                        self.unexpected(&token);
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag!(</col>)) => self.unexpected(&token),

                Token::Tag(tag!(<template> | </template>)) => {
                    self.step(InsertionMode::InHead, token)
                },

                Token::Eof => self.step(InsertionMode::InBody, token),

                token => {
                    if self.current_node_named(local_name!("colgroup")) {
                        self.pop();
                        ProcessResult::Reprocess(InsertionMode::InTable, token)
                    } else {
                        self.unexpected(&token)
                    }
                },
            },

            // § parsing-main-intbody
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intbody>
            InsertionMode::InTableBody => match token {
                Token::Tag(tag @ tag!(<tr>)) => {
                    self.pop_until_current(table_body_context);
                    self.insert_element_for(tag);
                    self.mode.set(InsertionMode::InRow);
                    ProcessResult::Done
                },

                Token::Tag(tag!(<th> | <td>)) => {
                    self.unexpected(&token);
                    self.pop_until_current(table_body_context);
                    self.insert_phantom(local_name!("tr"));
                    ProcessResult::Reprocess(InsertionMode::InRow, token)
                },

                Token::Tag(tag @ tag!(</tbody> | </tfoot> | </thead>)) => {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.pop_until_current(table_body_context);
                        self.pop();
                        self.mode.set(InsertionMode::InTable);
                    } else {
                        self.unexpected(&tag);
                    }
                    ProcessResult::Done
                },

                Token::Tag(
                    tag!(<caption> | <col> | <colgroup> | <tbody> | <tfoot> | <thead> | </table>),
                ) => {
                    declare_tag_set!(table_outer = "table" "tbody" "tfoot");
                    if self.in_scope(table_scope, |e| self.elem_in(&e, table_outer)) {
                        self.pop_until_current(table_body_context);
                        self.pop();
                        ProcessResult::Reprocess(InsertionMode::InTable, token)
                    } else {
                        self.unexpected(&token)
                    }
                },

                Token::Tag(
                    tag!(</body> | </caption> | </col> | </colgroup> | </html> | </td> | </th> | </tr>),
                ) => self.unexpected(&token),

                token => self.step(InsertionMode::InTable, token),
            },

            // § parsing-main-intr
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intr>
            InsertionMode::InRow => match token {
                Token::Tag(tag @ tag!(<th> | <td>)) => {
                    self.pop_until_current(table_row_context);
                    self.insert_element_for(tag);
                    self.mode.set(InsertionMode::InCell);
                    self.active_formatting
                        .borrow_mut()
                        .push(FormatEntry::Marker);
                    ProcessResult::Done
                },

                Token::Tag(tag!(</tr>)) => {
                    if self.in_scope_named(table_scope, local_name!("tr")) {
                        self.pop_until_current(table_row_context);
                        let node = self.pop();
                        self.assert_named(&node, local_name!("tr"));
                        self.mode.set(InsertionMode::InTableBody);
                    } else {
                        self.unexpected(&token);
                    }
                    ProcessResult::Done
                },

                Token::Tag(
                    tag!(<caption> | <col> | <colgroup> | <tbody> | <tfoot> | <thead> | <tr> | </table>),
                ) => {
                    if self.in_scope_named(table_scope, local_name!("tr")) {
                        self.pop_until_current(table_row_context);
                        let node = self.pop();
                        self.assert_named(&node, local_name!("tr"));
                        ProcessResult::Reprocess(InsertionMode::InTableBody, token)
                    } else {
                        self.unexpected(&token)
                    }
                },

                Token::Tag(tag @ tag!(</tbody> | </tfoot> | </thead>)) => {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        if self.in_scope_named(table_scope, local_name!("tr")) {
                            self.pop_until_current(table_row_context);
                            let node = self.pop();
                            self.assert_named(&node, local_name!("tr"));
                            ProcessResult::Reprocess(InsertionMode::InTableBody, Token::Tag(tag))
                        } else {
                            ProcessResult::Done
                        }
                    } else {
                        self.unexpected(&tag)
                    }
                },

                Token::Tag(
                    tag!(</body> | </caption> | </col> | </colgroup> | </html> | </td> | </th>),
                ) => self.unexpected(&token),

                token => self.step(InsertionMode::InTable, token),
            },

            // § parsing-main-intd
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intd>
            InsertionMode::InCell => match token {
                Token::Tag(tag @ tag!(</td> | </th>)) => {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.generate_implied_end_tags(cursory_implied_end);
                        self.expect_to_close(tag.name);
                        self.clear_active_formatting_to_marker();
                        self.mode.set(InsertionMode::InRow);
                    } else {
                        self.unexpected(&tag);
                    }
                    ProcessResult::Done
                },

                Token::Tag(
                    tag!(<caption> | <col> | <colgroup> | <tbody> | <td> | <tfoot> | <th> | <thead> | <tr>),
                ) => {
                    if self.in_scope(table_scope, |n| self.elem_in(&n, td_th)) {
                        self.close_the_cell();
                        ProcessResult::Reprocess(InsertionMode::InRow, token)
                    } else {
                        self.unexpected(&token)
                    }
                },

                Token::Tag(tag!(</body> | </caption> | </col> | </colgroup> | </html>)) => {
                    self.unexpected(&token)
                },

                Token::Tag(tag @ tag!(</table> | </tbody> | </tfoot> | </thead> | </tr>)) => {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.close_the_cell();
                        ProcessResult::Reprocess(InsertionMode::InRow, Token::Tag(tag))
                    } else {
                        self.unexpected(&tag)
                    }
                },

                token => self.step(InsertionMode::InBody, token),
            },

            // § parsing-main-inselect
            // TODO: not in spec?
            InsertionMode::InSelect => match token {
                Token::NullCharacter => self.unexpected(&token),
                Token::Characters(_, text) => self.append_text(text),
                Token::Comment(text) => self.append_comment(text),

                Token::Tag(tag!(<html>)) => self.step(InsertionMode::InBody, token),

                Token::Tag(tag @ tag!(<option>)) => {
                    if self.current_node_named(local_name!("option")) {
                        self.pop();
                    }
                    self.insert_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<optgroup>)) => {
                    if self.current_node_named(local_name!("option")) {
                        self.pop();
                    }
                    if self.current_node_named(local_name!("optgroup")) {
                        self.pop();
                    }
                    self.insert_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<hr>)) => {
                    if self.current_node_named(local_name!("option")) {
                        self.pop();
                    }
                    if self.current_node_named(local_name!("optgroup")) {
                        self.pop();
                    }
                    self.insert_element_for(tag);
                    self.pop();
                    ProcessResult::DoneAckSelfClosing
                },

                Token::Tag(tag!( </optgroup>)) => {
                    if self.open_elems.borrow().len() >= 2
                        && self.current_node_named(local_name!("option"))
                        && self.html_elem_named(
                            &self.open_elems.borrow()[self.open_elems.borrow().len() - 2],
                            local_name!("optgroup"),
                        )
                    {
                        self.pop();
                    }
                    if self.current_node_named(local_name!("optgroup")) {
                        self.pop();
                    } else {
                        self.unexpected(&token);
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag!(</option>)) => {
                    if self.current_node_named(local_name!("option")) {
                        self.pop();
                    } else {
                        self.unexpected(&token);
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<select> | </select>)) => {
                    let in_scope = self.in_scope_named(select_scope, local_name!("select"));

                    if !in_scope || tag.kind == StartTag {
                        self.unexpected(&tag);
                    }

                    if in_scope {
                        self.pop_until_named(local_name!("select"));
                        self.mode.set(self.reset_insertion_mode());
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag!(<input> | <keygen> | <textarea>)) => {
                    self.unexpected(&token);
                    if self.in_scope_named(select_scope, local_name!("select")) {
                        self.pop_until_named(local_name!("select"));
                        ProcessResult::Reprocess(self.reset_insertion_mode(), token)
                    } else {
                        ProcessResult::Done
                    }
                },

                Token::Tag(tag!(<script> | <template> | </template>)) => {
                    self.step(InsertionMode::InHead, token)
                },

                Token::Eof => self.step(InsertionMode::InBody, token),

                token => self.unexpected(&token),
            },

            // § parsing-main-inselectintable
            // TODO: not in spec?
            InsertionMode::InSelectInTable => match_token!(token {
                Token::Tag(tag!(<caption> | <table> | <tbody> | <tfoot> | <thead> | <tr> | <td> | <th>)) => {
                    self.unexpected(&token);
                    self.pop_until_named(local_name!("select"));
                    ProcessResult::Reprocess(self.reset_insertion_mode(), token)
                }

                Token::Tag(tag @ tag!(</caption> | </table> | </tbody> | </tfoot> | </thead> | </tr> | </td> | </th>)) => {
                    self.unexpected(&tag);
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.pop_until_named(local_name!("select"));
                        ProcessResult::Reprocess(self.reset_insertion_mode(), Token::Tag(tag))
                    } else {
                        ProcessResult::Done
                    }
                }

                token => self.step(InsertionMode::InSelect, token),
            }),

            // § parsing-main-intemplate
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intemplate>
            InsertionMode::InTemplate => match token {
                Token::Characters(_, _) => self.step(InsertionMode::InBody, token),
                Token::Comment(_) => self.step(InsertionMode::InBody, token),

                Token::Tag(
                    tag!(<base> | <basefont> | <bgsound> | <link> | <meta> | <noframes> | <script> |
                                <style> | <template> | <title> | </template>),
                ) => self.step(InsertionMode::InHead, token),

                Token::Tag(tag!(<caption> | <colgroup> | <tbody> | <tfoot> | <thead>)) => {
                    self.template_modes.borrow_mut().pop();
                    self.template_modes
                        .borrow_mut()
                        .push(InsertionMode::InTable);
                    ProcessResult::Reprocess(InsertionMode::InTable, token)
                },

                Token::Tag(tag!(<col>)) => {
                    self.template_modes.borrow_mut().pop();
                    self.template_modes
                        .borrow_mut()
                        .push(InsertionMode::InColumnGroup);
                    ProcessResult::Reprocess(InsertionMode::InColumnGroup, token)
                },

                Token::Tag(tag!(<tr>)) => {
                    self.template_modes.borrow_mut().pop();
                    self.template_modes
                        .borrow_mut()
                        .push(InsertionMode::InTableBody);
                    ProcessResult::Reprocess(InsertionMode::InTableBody, token)
                },

                Token::Tag(tag!(<td> | <th>)) => {
                    self.template_modes.borrow_mut().pop();
                    self.template_modes.borrow_mut().push(InsertionMode::InRow);
                    ProcessResult::Reprocess(InsertionMode::InRow, token)
                },

                Token::Eof => {
                    if !self.in_html_elem_named(local_name!("template")) {
                        self.stop_parsing()
                    } else {
                        self.unexpected(&token);
                        self.pop_until_named(local_name!("template"));
                        self.clear_active_formatting_to_marker();
                        self.template_modes.borrow_mut().pop();
                        self.mode.set(self.reset_insertion_mode());
                        ProcessResult::Reprocess(self.reset_insertion_mode(), token)
                    }
                },

                Token::Tag(tag @ tag!(<>)) => {
                    self.template_modes.borrow_mut().pop();
                    self.template_modes.borrow_mut().push(InsertionMode::InBody);
                    ProcessResult::Reprocess(InsertionMode::InBody, Token::Tag(tag))
                },

                token => self.unexpected(&token),
            },

            // § parsing-main-afterbody
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterbody>
            InsertionMode::AfterBody => match token {
                Token::Characters(SplitStatus::NotSplit, text) => {
                    ProcessResult::SplitWhitespace(text)
                },
                Token::Characters(SplitStatus::Whitespace, _) => {
                    self.step(InsertionMode::InBody, token)
                },
                Token::Comment(text) => self.append_comment_to_html(text),

                Token::Tag(tag!(<html>)) => self.step(InsertionMode::InBody, token),

                Token::Tag(tag!(</html>)) => {
                    if self.is_fragment() {
                        self.unexpected(&token);
                    } else {
                        self.mode.set(InsertionMode::AfterAfterBody);
                    }
                    ProcessResult::Done
                },

                Token::Eof => self.stop_parsing(),

                token => {
                    self.unexpected(&token);
                    ProcessResult::Reprocess(InsertionMode::InBody, token)
                },
            },

            // § parsing-main-inframeset
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inframeset>
            InsertionMode::InFrameset => match token {
                Token::Characters(SplitStatus::NotSplit, text) => {
                    ProcessResult::SplitWhitespace(text)
                },
                Token::Characters(SplitStatus::Whitespace, text) => self.append_text(text),
                Token::Comment(text) => self.append_comment(text),

                Token::Tag(tag!(<html>)) => self.step(InsertionMode::InBody, token),

                Token::Tag(tag @ tag!(<frameset>)) => {
                    self.insert_element_for(tag);
                    ProcessResult::Done
                },

                Token::Tag(tag!(</frameset>)) => {
                    if self.open_elems.borrow().len() == 1 {
                        self.unexpected(&token);
                    } else {
                        self.pop();
                        if !self.is_fragment() && !self.current_node_named(local_name!("frameset"))
                        {
                            self.mode.set(InsertionMode::AfterFrameset);
                        }
                    }
                    ProcessResult::Done
                },

                Token::Tag(tag @ tag!(<frame>)) => {
                    self.insert_and_pop_element_for(tag);
                    ProcessResult::DoneAckSelfClosing
                },

                Token::Tag(tag!(<noframes>)) => self.step(InsertionMode::InHead, token),

                Token::Eof => {
                    if self.open_elems.borrow().len() != 1 {
                        self.unexpected(&token);
                    }
                    self.stop_parsing()
                },

                token => self.unexpected(&token),
            },

            // § parsing-main-afterframeset
            // <html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterframeset>
            InsertionMode::AfterFrameset => match token {
                Token::Characters(SplitStatus::NotSplit, text) => {
                    ProcessResult::SplitWhitespace(text)
                },
                Token::Characters(SplitStatus::Whitespace, text) => self.append_text(text),
                Token::Comment(text) => self.append_comment(text),

                Token::Tag(tag!(<html>)) => self.step(InsertionMode::InBody, token),

                Token::Tag(tag!(</html>)) => {
                    self.mode.set(InsertionMode::AfterAfterFrameset);
                    ProcessResult::Done
                },

                Token::Tag(tag!(<noframes>)) => self.step(InsertionMode::InHead, token),

                Token::Eof => self.stop_parsing(),

                token => self.unexpected(&token),
            },

            // § the-after-after-body-insertion-mode
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterframeset>
            InsertionMode::AfterAfterBody => match token {
                Token::Characters(SplitStatus::NotSplit, text) => {
                    ProcessResult::SplitWhitespace(text)
                },
                Token::Characters(SplitStatus::Whitespace, _) => {
                    self.step(InsertionMode::InBody, token)
                },
                Token::Comment(text) => self.append_comment_to_doc(text),

                Token::Tag(tag!(<html>)) => self.step(InsertionMode::InBody, token),

                Token::Eof => self.stop_parsing(),

                token => {
                    self.unexpected(&token);
                    ProcessResult::Reprocess(InsertionMode::InBody, token)
                },
            },

            // § the-after-after-frameset-insertion-mode
            // <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterframeset>
            InsertionMode::AfterAfterFrameset => match token {
                Token::Characters(SplitStatus::NotSplit, text) => {
                    ProcessResult::SplitWhitespace(text)
                },
                Token::Characters(SplitStatus::Whitespace, _) => {
                    self.step(InsertionMode::InBody, token)
                },
                Token::Comment(text) => self.append_comment_to_doc(text),

                Token::Tag(tag!(<html>)) => self.step(InsertionMode::InBody, token),

                Token::Eof => self.stop_parsing(),

                Token::Tag(tag!(<noframes>)) => self.step(InsertionMode::InHead, token),

                token => self.unexpected(&token),
            },
        }
    }

    /// § The rules for parsing tokens in foreign content
    /// <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-afterframeset>
    pub(crate) fn step_foreign(&self, token: Token) -> ProcessResult<Handle> {
        match token {
            Token::NullCharacter => {
                self.unexpected(&token);
                self.append_text("\u{fffd}".to_tendril())
            },

            Token::Characters(_, text) => {
                if any_not_whitespace(&text) {
                    self.frameset_ok.set(false);
                }
                self.append_text(text)
            },

            Token::Comment(text) => self.append_comment(text),

            Token::Tag(
                tag @
                tag!(<b> | <big> | <blockquote> | <body> | <br> | <center> | <code> | <dd> | <div> | <dl> |
                <dt> | <em> | <embed> | <h1> | <h2> | <h3> | <h4> | <h5> | <h6> | <head> | <hr> | <i> |
                <img> | <li> | <listing> | <menu> | <meta> | <nobr> | <ol> | <p> | <pre> | <ruby> |
                <s> | <small> | <span> | <strong> | <strike> | <sub> | <sup> | <table> | <tt> |
                <u> | <ul> | <var> | </br> | </p>),
            ) => self.unexpected_start_tag_in_foreign_content(tag),

            Token::Tag(tag @ tag!(<font>)) => {
                let unexpected = tag.attrs.iter().any(|attr| {
                    matches!(
                        attr.name.expanded(),
                        expanded_name!("", "color")
                            | expanded_name!("", "face")
                            | expanded_name!("", "size")
                    )
                });
                if unexpected {
                    self.unexpected_start_tag_in_foreign_content(tag)
                } else {
                    self.foreign_start_tag(tag)
                }
            },

            Token::Tag(tag @ tag!(<>)) => self.foreign_start_tag(tag),

            // FIXME(#118): </script> in SVG
            Token::Tag(tag @ tag!(</>)) => {
                let mut first = true;
                let mut stack_idx = self.open_elems.borrow().len() - 1;
                loop {
                    if stack_idx == 0 {
                        return ProcessResult::Done;
                    }

                    let html;
                    let eq;
                    {
                        let open_elems = self.open_elems.borrow();
                        let node_name = self.sink.elem_name(&open_elems[stack_idx]);
                        html = *node_name.ns() == ns!(html);
                        eq = node_name.local_name().eq_ignore_ascii_case(&tag.name);
                    }
                    if !first && html {
                        let mode = self.mode.get();
                        return self.step(mode, Token::Tag(tag));
                    }

                    if eq {
                        self.open_elems.borrow_mut().truncate(stack_idx);
                        return ProcessResult::Done;
                    }

                    if first {
                        self.unexpected(&tag);
                        first = false;
                    }
                    stack_idx -= 1;
                }
            },

            // FIXME: Why is this unreachable?
            Token::Eof => panic!("impossible case in foreign content"),
        }
    }
}
