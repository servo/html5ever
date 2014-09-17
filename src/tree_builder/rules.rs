// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The tree builder rules, as a single, enormous nested match expression.

use core::prelude::*;

use tree_builder::types::*;
use tree_builder::tag_sets::*;
use tree_builder::actions::TreeBuilderActions;
use tree_builder::interface::{TreeSink, Quirks, AppendNode};

use tokenizer::{Tag, StartTag, EndTag};
use tokenizer::states::{Rcdata, Rawtext, ScriptData, Plaintext};

use util::str::is_ascii_whitespace;

use core::mem::replace;
use collections::MutableSeq;
use collections::string::String;
use collections::str::Slice;

use string_cache::Atom;

fn any_not_whitespace(x: &String) -> bool {
    // FIXME: this might be much faster as a byte scan
    x.as_slice().chars().any(|c| !is_ascii_whitespace(c))
}

// This goes in a trait so that we can control visibility.
pub trait TreeBuilderStep<Handle> {
    fn step(&mut self, mode: InsertionMode, token: Token) -> ProcessResult;
}

#[doc(hidden)]
impl<'sink, Handle: Clone, Sink: TreeSink<Handle>>
    TreeBuilderStep<Handle> for super::TreeBuilder<'sink, Handle, Sink> {

    fn step(&mut self, mode: InsertionMode, token: Token) -> ProcessResult {
        self.debug_step(mode, &token);

        match mode {
            //§ the-initial-insertion-mode
            Initial => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => Done,
                CommentToken(text) => self.append_comment_to_doc(text),
                token => {
                    if !self.opts.iframe_srcdoc {
                        self.unexpected(&token);
                        self.set_quirks_mode(Quirks);
                    }
                    Reprocess(BeforeHtml, token)
                }
            }),

            //§ the-before-html-insertion-mode
            BeforeHtml => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => Done,
                CommentToken(text) => self.append_comment_to_doc(text),

                tag @ <html> => {
                    self.create_root(tag.attrs);
                    self.mode = BeforeHead;
                    Done
                }

                </head> </body> </html> </br> => else,

                tag @ </_> => self.unexpected(&tag),

                token => {
                    self.create_root(vec!());
                    Reprocess(BeforeHead, token)
                }
            }),

            //§ the-before-head-insertion-mode
            BeforeHead => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => Done,
                CommentToken(text) => self.append_comment(text),

                <html> => self.step(InBody, token),

                tag @ <head> => {
                    self.head_elem = Some(self.insert_element_for(tag));
                    self.mode = InHead;
                    Done
                }

                </head> </body> </html> </br> => else,

                tag @ </_> => self.unexpected(&tag),

                token => {
                    self.head_elem = Some(self.insert_phantom(atom!(head)));
                    Reprocess(InHead, token)
                }
            }),

            //§ parsing-main-inhead
            InHead => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => self.append_text(text),
                CommentToken(text) => self.append_comment(text),

                <html> => self.step(InBody, token),

                tag @ <base> <basefont> <bgsound> <link> <meta> => {
                    // FIXME: handle <meta charset=...> and <meta http-equiv="Content-Type">
                    self.insert_and_pop_element_for(tag);
                    DoneAckSelfClosing
                }

                tag @ <title> => {
                    self.parse_raw_data(tag, Rcdata);
                    Done
                }

                tag @ <noframes> <style> <noscript> => {
                    if (!self.opts.scripting_enabled) && (tag.name == atom!(noscript)) {
                        self.insert_element_for(tag);
                        self.mode = InHeadNoscript;
                    } else {
                        self.parse_raw_data(tag, Rawtext);
                    }
                    Done
                }

                tag @ <script> => {
                    let elem = self.sink.create_element(ns!(HTML), atom!(script), tag.attrs);
                    if self.opts.fragment {
                        self.sink.mark_script_already_started(elem.clone());
                    }
                    self.insert_appropriately(AppendNode(elem.clone()));
                    self.open_elems.push(elem);
                    self.to_raw_text_mode(ScriptData);
                    Done
                }

                </head> => {
                    self.pop();
                    self.mode = AfterHead;
                    Done
                }

                </body> </html> </br> => else,

                <template> => fail!("FIXME: <template> not implemented"),
                </template> => fail!("FIXME: <template> not implemented"),

                <head> => self.unexpected(&token),
                tag @ </_> => self.unexpected(&tag),

                token => {
                    self.pop();
                    Reprocess(AfterHead, token)
                }
            }),

            //§ parsing-main-inheadnoscript
            InHeadNoscript => match_token!(token {
                <html> => self.step(InBody, token),

                </noscript> => {
                    self.pop();
                    self.mode = InHead;
                    Done
                },

                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InHead, token),

                CommentToken(_) => self.step(InHead, token),

                <basefont> <bgsound> <link> <meta> <noframes> <style>
                    => self.step(InHead, token),

                </br> => else,

                <head> <noscript> => self.unexpected(&token),
                tag @ </_> => self.unexpected(&tag),

                token => {
                    self.unexpected(&token);
                    self.pop();
                    Reprocess(InHead, token)
                },
            }),

            //§ the-after-head-insertion-mode
            AfterHead => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => self.append_text(text),
                CommentToken(text) => self.append_comment(text),

                <html> => self.step(InBody, token),

                tag @ <body> => {
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    self.mode = InBody;
                    Done
                }

                tag @ <frameset> => {
                    self.insert_element_for(tag);
                    self.mode = InFrameset;
                    Done
                }

                <base> <basefont> <bgsound> <link> <meta>
                      <noframes> <script> <style> <template> <title> => {
                    self.unexpected(&token);
                    let head = self.head_elem.as_ref().expect("no head element").clone();
                    self.push(&head);
                    let result = self.step(InHead, token);
                    self.remove_from_stack(&head);
                    result
                }

                </template> => self.step(InHead, token),

                </body> </html> </br> => else,

                <head> => self.unexpected(&token),
                tag @ </_> => self.unexpected(&tag),

                token => {
                    self.insert_phantom(atom!(body));
                    Reprocess(InBody, token)
                }
            }),

            //§ parsing-main-inbody
            InBody => match_token!(token {
                NullCharacterToken => self.unexpected(&token),

                CharacterTokens(_, text) => {
                    self.reconstruct_formatting();
                    if any_not_whitespace(&text) {
                        self.frameset_ok = false;
                    }
                    self.append_text(text)
                }

                CommentToken(text) => self.append_comment(text),

                tag @ <html> => {
                    self.unexpected(&tag);
                    // FIXME: <template>
                    let top = self.html_elem();
                    self.sink.add_attrs_if_missing(top, tag.attrs);
                    Done
                }

                <base> <basefont> <bgsound> <link> <meta> <noframes>
                  <script> <style> <template> <title> </template> => {
                    self.step(InHead, token)
                }

                tag @ <body> => {
                    self.unexpected(&tag);
                    // FIXME: <template>
                    match self.body_elem() {
                        None => (),
                        Some(node) => {
                            self.frameset_ok = false;
                            self.sink.add_attrs_if_missing(node, tag.attrs)
                        }
                    }
                    Done
                }

                tag @ <frameset> => {
                    self.unexpected(&tag);
                    if !self.frameset_ok { return Done; }

                    // Can't use unwrap_or_return!() due to rust-lang/rust#16617.
                    let body = match self.body_elem() {
                        None => return Done,
                        Some(x) => x,
                    };
                    self.sink.remove_from_parent(body);

                    // FIXME: can we get here in the fragment case?
                    // What to do with the first element then?
                    self.open_elems.truncate(1);
                    self.insert_element_for(tag);
                    self.mode = InFrameset;
                    Done
                }

                EOFToken => {
                    // FIXME: <template>
                    self.check_body_end();
                    self.stop_parsing()
                }

                </body> => {
                    if self.in_scope_named(default_scope, atom!(body)) {
                        self.check_body_end();
                        self.mode = AfterBody;
                    } else {
                        self.sink.parse_error(Slice("</body> with no <body> in scope"));
                    }
                    Done
                }

                </html> => {
                    if self.in_scope_named(default_scope, atom!(body)) {
                        self.check_body_end();
                        Reprocess(AfterBody, token)
                    } else {
                        self.sink.parse_error(Slice("</html> with no <body> in scope"));
                        Done
                    }
                }

                tag @ <address> <article> <aside> <blockquote> <center> <details> <dialog>
                  <dir> <div> <dl> <fieldset> <figcaption> <figure> <footer> <header>
                  <hgroup> <main> <menu> <nav> <ol> <p> <section> <summary> <ul> => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    Done
                }

                tag @ <h1> <h2> <h3> <h4> <h5> <h6> => {
                    self.close_p_element_in_button_scope();
                    if self.current_node_in(heading_tag) {
                        self.sink.parse_error(Slice("nested heading tags"));
                        self.pop();
                    }
                    self.insert_element_for(tag);
                    Done
                }

                tag @ <pre> <listing> => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    self.ignore_lf = true;
                    self.frameset_ok = false;
                    Done
                }

                tag @ <form> => {
                    // FIXME: <template>
                    if self.form_elem.is_some() {
                        self.sink.parse_error(Slice("nested forms"));
                    } else {
                        self.close_p_element_in_button_scope();
                        let elem = self.insert_element_for(tag);
                        // FIXME: <template>
                        self.form_elem = Some(elem);
                    }
                    Done
                }

                tag @ <li> <dd> <dt> => {
                    declare_tag_set!(close_list = li)
                    declare_tag_set!(close_defn = dd dt)
                    declare_tag_set!(extra_special = special_tag - address div p)
                    let can_close = match tag.name {
                        atom!(li) => close_list,
                        atom!(dd) | atom!(dt) => close_defn,
                        _ => unreachable!(),
                    };

                    self.frameset_ok = false;

                    let mut to_close = None;
                    for node in self.open_elems.iter().rev() {
                        let nsname = self.sink.elem_name(node.clone());
                        if can_close(nsname.clone()) {
                            let (_, name) = nsname;
                            to_close = Some(name);
                            break;
                        }
                        if extra_special(nsname.clone()) {
                            break;
                        }
                    }

                    match to_close {
                        Some(name) => {
                            self.generate_implied_end_except(name.clone());
                            self.expect_to_close(name);
                        }
                        None => (),
                    }

                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    Done
                }

                tag @ <plaintext> => {
                    self.close_p_element_in_button_scope();
                    self.insert_element_for(tag);
                    self.next_tokenizer_state = Some(Plaintext);
                    Done
                }

                tag @ <button> => {
                    if self.in_scope_named(default_scope, atom!(button)) {
                        self.sink.parse_error(Slice("nested buttons"));
                        self.generate_implied_end(cursory_implied_end);
                        self.pop_until_named(atom!(button));
                    }
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    Done
                }

                tag @ </address> </article> </aside> </blockquote> </button> </center>
                  </details> </dialog> </dir> </div> </dl> </fieldset> </figcaption>
                  </figure> </footer> </header> </hgroup> </listing> </main> </menu>
                  </nav> </ol> </pre> </section> </summary> </ul> => {
                    if !self.in_scope_named(default_scope, tag.name.clone()) {
                        self.unexpected(&tag);
                    } else {
                        self.generate_implied_end(cursory_implied_end);
                        self.expect_to_close(tag.name);
                    }
                    Done
                }

                </form> => {
                    // FIXME: <template>
                    // Can't use unwrap_or_return!() due to rust-lang/rust#16617.
                    let node = match self.form_elem.take() {
                        None => {
                            self.sink.parse_error(Slice("Null form element pointer on </form>"));
                            return Done;
                        }
                        Some(x) => x,
                    };
                    if !self.in_scope(default_scope,
                        |n| self.sink.same_node(node.clone(), n)) {
                        self.sink.parse_error(Slice("Form element not in scope on </form>"));
                        return Done;
                    }
                    self.generate_implied_end(cursory_implied_end);
                    let current = self.current_node();
                    self.remove_from_stack(&node);
                    if !self.sink.same_node(current, node) {
                        self.sink.parse_error(Slice("Bad open element on </form>"));
                    }
                    Done
                }

                </p> => {
                    if !self.in_scope_named(button_scope, atom!(p)) {
                        self.sink.parse_error(Slice("No <p> tag to close"));
                        self.insert_phantom(atom!(p));
                    }
                    self.close_p_element();
                    Done
                }

                tag @ </li> </dd> </dt> => {
                    let scope = match tag.name {
                        atom!(li) => list_item_scope,
                        _ => default_scope,
                    };
                    if self.in_scope_named(|x| scope(x), tag.name.clone()) {
                        self.generate_implied_end_except(tag.name.clone());
                        self.expect_to_close(tag.name);
                    } else {
                        self.sink.parse_error(Slice("No matching tag to close"));
                    }
                    Done
                }

                tag @ </h1> </h2> </h3> </h4> </h5> </h6> => {
                    if self.in_scope(default_scope, |n| self.elem_in(n.clone(), heading_tag)) {
                        self.generate_implied_end(cursory_implied_end);
                        if !self.current_node_named(tag.name) {
                            self.sink.parse_error(Slice("Closing wrong heading tag"));
                        }
                        self.pop_until(heading_tag);
                    } else {
                        self.sink.parse_error(Slice("No heading tag to close"));
                    }
                    Done
                }

                tag @ <a> => {
                    let mut to_remove = vec!();
                    for (i, handle, _) in self.active_formatting_end_to_marker() {
                        if self.html_elem_named(handle.clone(), atom!(a)) {
                            to_remove.push((i, handle.clone()));
                        }
                    }

                    if !to_remove.is_empty() {
                        self.unexpected(&tag);
                        self.adoption_agency(atom!(a));
                        // FIXME: quadratic time
                        for (i, handle) in to_remove.into_iter() {
                            self.remove_from_stack(&handle);
                            self.active_formatting.remove(i);
                            // We iterated backwards from the end above, so
                            // we don't need to adjust the indices after each
                            // removal.
                        }
                    }

                    self.reconstruct_formatting();
                    self.create_formatting_element_for(tag);
                    Done
                }

                tag @ <b> <big> <code> <em> <font> <i> <s> <small> <strike> <strong> <tt> <u> => {
                    self.reconstruct_formatting();
                    self.create_formatting_element_for(tag);
                    Done
                }

                tag @ <nobr> => {
                    self.reconstruct_formatting();
                    if self.in_scope_named(default_scope, atom!(nobr)) {
                        self.sink.parse_error(Slice("Nested <nobr>"));
                        self.adoption_agency(atom!(nobr));
                        self.reconstruct_formatting();
                    }
                    self.create_formatting_element_for(tag);
                    Done
                }

                tag @ </a> </b> </big> </code> </em> </font> </i> </nobr>
                  </s> </small> </strike> </strong> </tt> </u> => {
                    self.adoption_agency(tag.name);
                    Done
                }

                tag @ <applet> <marquee> <object> => {
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    self.active_formatting.push(Marker);
                    self.frameset_ok = false;
                    Done
                }

                tag @ </applet> </marquee> </object> => {
                    if !self.in_scope_named(default_scope, tag.name.clone()) {
                        self.unexpected(&tag);
                    } else {
                        self.generate_implied_end(cursory_implied_end);
                        self.expect_to_close(tag.name);
                        self.clear_active_formatting_to_marker();
                    }
                    Done
                }

                tag @ <table> => {
                    if self.quirks_mode != Quirks {
                        self.close_p_element_in_button_scope();
                    }
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    self.mode = InTable;
                    Done
                }

                tag @ </br> => {
                    self.unexpected(&tag);
                    self.step(InBody, TagToken(Tag {
                        kind: StartTag,
                        attrs: vec!(),
                        ..tag
                    }))
                }

                tag @ <area> <br> <embed> <img> <keygen> <wbr> <input> => {
                    let keep_frameset_ok = match tag.name {
                        atom!(input) => self.is_type_hidden(&tag),
                        _ => false,
                    };
                    self.reconstruct_formatting();
                    self.insert_and_pop_element_for(tag);
                    if !keep_frameset_ok {
                        self.frameset_ok = false;
                    }
                    DoneAckSelfClosing
                }

                tag @ <menuitem> <param> <source> <track> => {
                    self.insert_and_pop_element_for(tag);
                    DoneAckSelfClosing
                }

                tag @ <hr> => {
                    self.close_p_element_in_button_scope();
                    self.insert_and_pop_element_for(tag);
                    self.frameset_ok = false;
                    DoneAckSelfClosing
                }

                tag @ <image> => {
                    self.unexpected(&tag);
                    self.step(InBody, TagToken(Tag {
                        name: atom!(img),
                        ..tag
                    }))
                }

                <isindex> => fail!("FIXME: <isindex> not implemented"),

                tag @ <textarea> => {
                    self.ignore_lf = true;
                    self.frameset_ok = false;
                    self.parse_raw_data(tag, Rcdata);
                    Done
                }

                tag @ <xmp> => {
                    self.close_p_element_in_button_scope();
                    self.reconstruct_formatting();
                    self.frameset_ok = false;
                    self.parse_raw_data(tag, Rawtext);
                    Done
                }

                tag @ <iframe> => {
                    self.frameset_ok = false;
                    self.parse_raw_data(tag, Rawtext);
                    Done
                }

                tag @ <noembed> => {
                    self.parse_raw_data(tag, Rawtext);
                    Done
                }

                // <noscript> handled in wildcard case below

                tag @ <select> => {
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    self.frameset_ok = false;
                    // NB: mode == InBody but possibly self.mode != mode, if
                    // we're processing "as in the rules for InBody".
                    self.mode = match self.mode {
                        InTable | InCaption | InTableBody
                            | InRow | InCell => InSelectInTable,
                        _ => InSelect,
                    };
                    Done
                }

                tag @ <optgroup> <option> => {
                    if self.current_node_named(atom!(option)) {
                        self.pop();
                    }
                    self.reconstruct_formatting();
                    self.insert_element_for(tag);
                    Done
                }

                tag @ <rp> <rt> => {
                    if self.in_scope_named(default_scope, atom!(ruby)) {
                        self.generate_implied_end(cursory_implied_end);
                    }
                    if !self.current_node_named(atom!(ruby)) {
                        self.unexpected(&tag);
                    }
                    self.insert_element_for(tag);
                    Done
                }

                <math> => fail!("FIXME: MathML not implemented"),
                <svg> => fail!("FIXME: SVG not implemented"),

                <caption> <col> <colgroup> <frame> <head>
                  <tbody> <td> <tfoot> <th> <thead> <tr> => {
                    self.unexpected(&token);
                    Done
                }

                tag @ <_> => {
                    if self.opts.scripting_enabled && tag.name == atom!(noscript) {
                        self.parse_raw_data(tag, Rawtext);
                    } else {
                        self.reconstruct_formatting();
                        self.insert_element_for(tag);
                    }
                    Done
                }

                tag @ </_> => {
                    // Look back for a matching open element.
                    let mut match_idx = None;
                    for (i, elem) in self.open_elems.iter().enumerate().rev() {
                        if self.html_elem_named(elem.clone(), tag.name.clone()) {
                            match_idx = Some(i);
                            break;
                        }

                        if self.elem_in(elem.clone(), special_tag) {
                            self.sink.parse_error(Slice("Found special tag while closing generic tag"));
                            return Done;
                        }
                    }

                    // Can't use unwrap_or_return!() due to rust-lang/rust#16617.
                    let match_idx = match match_idx {
                        None => {
                            // I believe this is impossible, because the root
                            // <html> element is in special_tag.
                            self.unexpected(&tag);
                            return Done;
                        }
                        Some(x) => x,
                    };

                    self.generate_implied_end_except(tag.name.clone());

                    if match_idx != self.open_elems.len() - 1 {
                        // mis-nested tags
                        self.unexpected(&tag);
                    }
                    self.open_elems.truncate(match_idx);
                    Done
                }

                // FIXME: This should be unreachable, but match_token! requires a
                // catch-all case.
                _ => fail!("impossible case in InBody mode"),
            }),

            //§ parsing-main-incdata
            Text => match_token!(token {
                CharacterTokens(_, text) => self.append_text(text),

                EOFToken => {
                    self.unexpected(&token);
                    if self.current_node_named(atom!(script)) {
                        let current = self.current_node();
                        self.sink.mark_script_already_started(current);
                    }
                    self.pop();
                    Reprocess(self.orig_mode.take_unwrap(), token)
                }

                tag @ </_> => {
                    if tag.name == atom!(script) {
                        h5e_warn!("FIXME: </script> not implemented");
                    }

                    self.pop();
                    self.mode = self.orig_mode.take_unwrap();
                    Done
                }

                // The spec doesn't say what to do here.
                // Other tokens are impossible?
                _ => fail!("impossible case in Text mode"),
            }),

            //§ parsing-main-intable
            InTable => match_token!(token {
                // FIXME: hack, should implement pat | pat for match_token!() instead
                NullCharacterToken => self.process_chars_in_table(token),

                CharacterTokens(..) => self.process_chars_in_table(token),

                CommentToken(text) => self.append_comment(text),

                tag @ <caption> => {
                    self.pop_until_current(table_scope);
                    self.active_formatting.push(Marker);
                    self.insert_element_for(tag);
                    self.mode = InCaption;
                    Done
                }

                tag @ <colgroup> => {
                    self.pop_until_current(table_scope);
                    self.insert_element_for(tag);
                    self.mode = InColumnGroup;
                    Done
                }

                <col> => {
                    self.pop_until_current(table_scope);
                    self.insert_phantom(atom!(colgroup));
                    Reprocess(InColumnGroup, token)
                }

                tag @ <tbody> <tfoot> <thead> => {
                    self.pop_until_current(table_scope);
                    self.insert_element_for(tag);
                    self.mode = InTableBody;
                    Done
                }

                <td> <th> <tr> => {
                    self.pop_until_current(table_scope);
                    self.insert_phantom(atom!(tbody));
                    Reprocess(InTableBody, token)
                }

                <table> => {
                    self.unexpected(&token);
                    if self.in_scope_named(table_scope, atom!(table)) {
                        self.pop_until_named(atom!(table));
                        Reprocess(self.reset_insertion_mode(), token)
                    } else {
                        Done
                    }
                }

                </table> => {
                    if self.in_scope_named(table_scope, atom!(table)) {
                        self.pop_until_named(atom!(table));
                        self.mode = self.reset_insertion_mode();
                    } else {
                        self.unexpected(&token);
                    }
                    Done
                }

                </body> </caption> </col> </colgroup> </html>
                  </tbody> </td> </tfoot> </th> </thead> </tr> =>
                    self.unexpected(&token),

                <style> <script> <template> </template>
                    => self.step(InHead, token),

                tag @ <input> => {
                    self.unexpected(&tag);
                    if self.is_type_hidden(&tag) {
                        self.insert_and_pop_element_for(tag);
                        DoneAckSelfClosing
                    } else {
                        self.foster_parent_in_body(TagToken(tag))
                    }
                }

                tag @ <form> => {
                    self.unexpected(&tag);
                    // FIXME: <template>
                    if self.form_elem.is_none() {
                        self.form_elem = Some(self.insert_and_pop_element_for(tag));
                    }
                    Done
                }

                EOFToken => self.step(InBody, token),

                token => {
                    self.unexpected(&token);
                    self.foster_parent_in_body(token)
                }
            }),

            //§ parsing-main-intabletext
            InTableText => match_token!(token {
                NullCharacterToken => self.unexpected(&token),

                CharacterTokens(split, text) => {
                    self.pending_table_text.push((split, text));
                    Done
                }

                token => {
                    let pending = replace(&mut self.pending_table_text, vec!());
                    let contains_nonspace = pending.iter().any(|&(split, ref text)| {
                        match split {
                            Whitespace => false,
                            NotWhitespace => true,
                            NotSplit => any_not_whitespace(text),
                        }
                    });

                    if contains_nonspace {
                        self.sink.parse_error(Slice("Non-space table text"));
                        for (split, text) in pending.into_iter() {
                            match self.foster_parent_in_body(CharacterTokens(split, text)) {
                                Done => (),
                                _ => fail!("not prepared to handle this!"),
                            }
                        }
                    } else {
                        for (_, text) in pending.into_iter() {
                            self.append_text(text);
                        }
                    }

                    Reprocess(self.orig_mode.take_unwrap(), token)
                }
            }),

            //§ parsing-main-incaption
            InCaption => match_token!(token {
                tag @ <caption> <col> <colgroup> <tbody> <td> <tfoot>
                  <th> <thead> <tr> </table> </caption> => {
                    if self.in_scope_named(table_scope, atom!(caption)) {
                        self.generate_implied_end(cursory_implied_end);
                        self.expect_to_close(atom!(caption));
                        self.clear_active_formatting_to_marker();
                        match tag {
                            Tag { kind: EndTag, name: atom!(caption), .. } => {
                                self.mode = InTable;
                                Done
                            }
                            _ => Reprocess(InTable, TagToken(tag))
                        }
                    } else {
                        self.unexpected(&tag);
                        Done
                    }
                }

                </body> </col> </colgroup> </html> </tbody>
                  </td> </tfoot> </th> </thead> </tr> => self.unexpected(&token),

                token => self.step(InBody, token),
            }),

            //§ parsing-main-incolgroup
            InColumnGroup => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => self.append_text(text),
                CommentToken(text) => self.append_comment_to_html(text),

                <html> => self.step(InBody, token),

                tag @ <col> => {
                    self.insert_and_pop_element_for(tag);
                    DoneAckSelfClosing
                }

                </colgroup> => {
                    if self.current_node_named(atom!(colgroup)) {
                        self.pop();
                        self.mode = InTable;
                    } else {
                        self.unexpected(&token);
                    }
                    Done
                }

                </col> => self.unexpected(&token),

                <template> </template> => self.step(InHead, token),

                EOFToken => self.step(InBody, token),

                token => {
                    if self.current_node_named(atom!(colgroup)) {
                        self.pop();
                    } else {
                        self.unexpected(&token);
                    }
                    Reprocess(InTable, token)
                }
            }),

            //§ parsing-main-intbody
            InTableBody => match_token!(token {
                tag @ <tr> => {
                    self.pop_until_current(table_body_context);
                    self.insert_element_for(tag);
                    self.mode = InRow;
                    Done
                }

                <th> <td> => {
                    self.unexpected(&token);
                    self.pop_until_current(table_body_context);
                    self.insert_phantom(atom!(tr));
                    Reprocess(InRow, token)
                }

                tag @ </tbody> </tfoot> </thead> => {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.pop_until_current(table_body_context);
                        self.pop();
                        self.mode = InTable;
                    } else {
                        self.unexpected(&tag);
                    }
                    Done
                }

                <caption> <col> <colgroup> <tbody> <tfoot> <thead> </table> => {
                    declare_tag_set!(table_outer = table tbody tfoot)
                    if self.in_scope(table_scope, |e| self.elem_in(e, table_outer)) {
                        self.pop_until_current(table_body_context);
                        self.pop();
                        Reprocess(InTable, token)
                    } else {
                        self.unexpected(&token)
                    }
                }

                </body> </caption> </col> </colgroup> </html> </td> </th> </tr>
                    => self.unexpected(&token),

                token => self.step(InTable, token),
            }),

            //§ parsing-main-intr
            InRow => match_token!(token {
                tag @ <th> <td> => {
                    self.pop_until_current(table_row_context);
                    self.insert_element_for(tag);
                    self.mode = InCell;
                    self.active_formatting.push(Marker);
                    Done
                }

                </tr> => {
                    if self.in_scope_named(table_scope, atom!(tr)) {
                        self.pop_until_current(table_row_context);
                        let node = self.pop();
                        self.assert_named(node, atom!(tr));
                        self.mode = InTableBody;
                    } else {
                        self.unexpected(&token);
                    }
                    Done
                }

                <caption> <col> <colgroup> <tbody> <tfoot> <thead> <tr> </table> => {
                    if self.in_scope_named(table_scope, atom!(tr)) {
                        self.pop_until_current(table_row_context);
                        let node = self.pop();
                        self.assert_named(node, atom!(tr));
                        Reprocess(InTableBody, token)
                    } else {
                        self.unexpected(&token)
                    }
                }

                tag @ </tbody> </tfoot> </thead> => {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        if self.in_scope_named(table_scope, atom!(tr)) {
                            self.pop_until_current(table_row_context);
                            let node = self.pop();
                            self.assert_named(node, atom!(tr));
                            Reprocess(InTableBody, TagToken(tag))
                        } else {
                            Done
                        }
                    } else {
                        self.unexpected(&tag)
                    }
                }

                </body> </caption> </col> </colgroup> </html> </td> </th>
                    => self.unexpected(&token),

                token => self.step(InTable, token),
            }),

            //§ parsing-main-intd
            InCell => match_token!(token {
                tag @ </td> </th> => {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.generate_implied_end(cursory_implied_end);
                        self.expect_to_close(tag.name);
                        self.clear_active_formatting_to_marker();
                        self.mode = InRow;
                    } else {
                        self.unexpected(&tag);
                    }
                    Done
                }

                <caption> <col> <colgroup> <tbody> <td> <tfoot> <th> <thead> <tr> => {
                    if self.in_scope(table_scope, |n| self.elem_in(n.clone(), td_th)) {
                        self.close_the_cell();
                        Reprocess(InRow, token)
                    } else {
                        self.unexpected(&token)
                    }
                }

                </body> </caption> </col> </colgroup> </html>
                    => self.unexpected(&token),

                tag @ </table> </tbody> </tfoot> </thead> </tr> => {
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.close_the_cell();
                        Reprocess(InRow, TagToken(tag))
                    } else {
                        self.unexpected(&tag)
                    }
                }

                token => self.step(InBody, token),
            }),

            //§ parsing-main-inselect
            InSelect => match_token!(token {
                NullCharacterToken => self.unexpected(&token),
                CharacterTokens(_, text) => self.append_text(text),
                CommentToken(text) => self.append_comment(text),

                <html> => self.step(InBody, token),

                tag @ <option> => {
                    if self.current_node_named(atom!(option)) {
                        self.pop();
                    }
                    self.insert_element_for(tag);
                    Done
                }

                tag @ <optgroup> => {
                    if self.current_node_named(atom!(option)) {
                        self.pop();
                    }
                    if self.current_node_named(atom!(optgroup)) {
                        self.pop();
                    }
                    self.insert_element_for(tag);
                    Done
                }

                </optgroup> => {
                    if self.open_elems.len() >= 2
                        && self.current_node_named(atom!(option))
                        && self.html_elem_named(self.open_elems.get(1).clone(),
                            atom!(optgroup)) {
                        self.pop();
                    }
                    if self.current_node_named(atom!(optgroup)) {
                        self.pop();
                    } else {
                        self.unexpected(&token);
                    }
                    Done
                }

                </option> => {
                    if self.current_node_named(atom!(option)) {
                        self.pop();
                    } else {
                        self.unexpected(&token);
                    }
                    Done
                }

                tag @ <select> </select> => {
                    let in_scope = self.in_scope_named(select_scope, atom!(select));

                    if !in_scope || tag.kind == StartTag {
                        self.unexpected(&tag);
                    }

                    if in_scope {
                        self.pop_until_named(atom!(select));
                        self.mode = self.reset_insertion_mode();
                    }
                    Done
                }

                <input> <keygen> <textarea> => {
                    self.unexpected(&token);
                    if self.in_scope_named(select_scope, atom!(select)) {
                        self.pop_until_named(atom!(select));
                        Reprocess(self.reset_insertion_mode(), token)
                    } else {
                        Done
                    }
                }

                <script> <template> </template> => self.step(InHead, token),

                EOFToken => self.step(InBody, token),

                token => self.unexpected(&token),
            }),

            //§ parsing-main-inselectintable
            InSelectInTable => match_token!(token {
                <caption> <table> <tbody> <tfoot> <thead> <tr> <td> <th> => {
                    self.unexpected(&token);
                    self.pop_until_named(atom!(select));
                    Reprocess(self.reset_insertion_mode(), token)
                }

                tag @ </caption> </table> </tbody> </tfoot> </thead> </tr> </td> </th> => {
                    self.unexpected(&tag);
                    if self.in_scope_named(table_scope, tag.name.clone()) {
                        self.pop_until_named(atom!(select));
                        Reprocess(self.reset_insertion_mode(), TagToken(tag))
                    } else {
                        Done
                    }
                }

                token => self.step(InSelect, token),
            }),

            //§ parsing-main-intemplate
            InTemplate
                => fail!("FIXME: <template> not implemented"),

            //§ parsing-main-afterbody
            AfterBody => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InBody, token),
                CommentToken(text) => self.append_comment_to_html(text),

                <html> => self.step(InBody, token),

                </html> => {
                    if self.opts.fragment {
                        self.unexpected(&token);
                    } else {
                        self.mode = AfterAfterBody;
                    }
                    Done
                }

                EOFToken => self.stop_parsing(),

                token => {
                    self.unexpected(&token);
                    Reprocess(InBody, token)
                }
            }),

            //§ parsing-main-inframeset
            InFrameset => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => self.append_text(text),
                CommentToken(text) => self.append_comment(text),

                <html> => self.step(InBody, token),

                tag @ <frameset> => {
                    self.insert_element_for(tag);
                    Done
                }

                </frameset> => {
                    if self.open_elems.len() == 1 {
                        self.unexpected(&token);
                    } else {
                        self.pop();
                        if !self.opts.fragment && !self.current_node_named(atom!(frameset)) {
                            self.mode = AfterFrameset;
                        }
                    }
                    Done
                }

                tag @ <frame> => {
                    self.insert_and_pop_element_for(tag);
                    DoneAckSelfClosing
                }

                <noframes> => self.step(InHead, token),

                EOFToken => {
                    if self.open_elems.len() != 1 {
                        self.unexpected(&token);
                    }
                    self.stop_parsing()
                }

                token => self.unexpected(&token),
            }),

            //§ parsing-main-afterframeset
            AfterFrameset => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, text) => self.append_text(text),
                CommentToken(text) => self.append_comment(text),

                <html> => self.step(InBody, token),

                </html> => {
                    self.mode = AfterAfterFrameset;
                    Done
                }

                <noframes> => self.step(InHead, token),

                EOFToken => self.stop_parsing(),

                token => self.unexpected(&token),
            }),

            //§ the-after-after-body-insertion-mode
            AfterAfterBody => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InBody, token),
                CommentToken(text) => self.append_comment_to_doc(text),

                <html> => self.step(InBody, token),

                EOFToken => self.stop_parsing(),

                token => {
                    self.unexpected(&token);
                    Reprocess(InBody, token)
                }
            }),

            //§ the-after-after-frameset-insertion-mode
            AfterAfterFrameset => match_token!(token {
                CharacterTokens(NotSplit, text) => SplitWhitespace(text),
                CharacterTokens(Whitespace, _) => self.step(InBody, token),
                CommentToken(text) => self.append_comment_to_doc(text),

                <html> => self.step(InBody, token),

                EOFToken => self.stop_parsing(),

                <noframes> => self.step(InHead, token),

                token => self.unexpected(&token),
            }),
            //§ END
        }
    }
}
