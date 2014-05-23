/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use util::atom::Atom;
use util::namespace::Namespace;
use tokenizer::Attribute;

#[deriving(Eq, TotalEq, Clone, Hash, Show)]
pub enum QuirksMode {
    Quirks,
    LimitedQuirks,
    NoQuirks,
}

pub trait TreeSink<Handle> {
    fn parse_error(&mut self, msg: ~str);
    fn get_document(&mut self) -> Handle;
    fn set_quirks_mode(&mut self, mode: QuirksMode);

    fn create_element(&mut self, ns: Namespace, name: Atom, attrs: Vec<Attribute>) -> Handle;

    fn append_text(&mut self, parent: Handle, text: StrBuf);
    fn append_comment(&mut self, parent: Handle, text: StrBuf);
    fn append_element(&mut self, parent: Handle, child: Handle);
    fn append_doctype_to_document(&mut self, name: StrBuf, public_id: StrBuf, system_id: StrBuf);

    fn mark_script_already_started(&mut self, node: Handle);
}
