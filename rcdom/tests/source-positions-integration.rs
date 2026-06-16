// Copyright 2014-2026 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Integration tests for the `source-positions` feature.
//!
//! Verifies that byte offsets flow correctly from `BufferQueue` through the
//! tokenizer and tree builder all the way into `TreeSink::set_current_byte`,
//! and that the offsets correspond to the actual positions of element opening
//! tags in the source string.

#[cfg(feature = "source-positions")]
mod source_positions {
    use html5ever::driver;
    use html5ever::tendril::stream::TendrilSink;
    use html5ever::tendril::StrTendril;
    use html5ever::ExpandedName;
    use html5ever::QualName;
    use markup5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
    use markup5ever::Attribute;
    use markup5ever_rcdom::{Handle, RcDom};
    use std::borrow::Cow;
    use std::cell::{Cell, RefCell};

    /// Wraps `RcDom` and records `(local_name, byte_offset)` for every
    /// element created while `set_current_byte` is active.
    struct ByteCapturingDOM {
        current_byte: Cell<u64>,
        elements: RefCell<Vec<(String, u64)>>,
        rcdom: RcDom,
    }

    impl ByteCapturingDOM {
        fn new() -> Self {
            ByteCapturingDOM {
                current_byte: Cell::new(0),
                elements: RefCell::new(vec![]),
                rcdom: RcDom::default(),
            }
        }

        /// Returns recorded `(local_name, byte_offset)` pairs, skipping the
        /// implicit wrapper elements html5ever inserts (`html`, `head`, `body`).
        fn content_elements(&self) -> Vec<(String, u64)> {
            self.elements
                .borrow()
                .iter()
                .filter(|(name, _)| !matches!(name.as_str(), "html" | "head" | "body"))
                .cloned()
                .collect()
        }
    }

    impl TreeSink for ByteCapturingDOM {
        type Output = Self;
        type ElemName<'a> = ExpandedName<'a>;

        fn finish(self) -> Self {
            self
        }

        type Handle = Handle;

        fn parse_error(&self, msg: Cow<'static, str>) {
            self.rcdom.parse_error(msg);
        }

        fn get_document(&self) -> Handle {
            self.rcdom.get_document()
        }

        fn get_template_contents(&self, target: &Handle) -> Handle {
            self.rcdom.get_template_contents(target)
        }

        fn set_quirks_mode(&self, mode: QuirksMode) {
            self.rcdom.set_quirks_mode(mode)
        }

        fn same_node(&self, x: &Handle, y: &Handle) -> bool {
            self.rcdom.same_node(x, y)
        }

        fn elem_name<'a>(&'a self, target: &'a Handle) -> ExpandedName<'a> {
            self.rcdom.elem_name(target)
        }

        fn create_element(
            &self,
            name: QualName,
            attrs: Vec<Attribute>,
            flags: ElementFlags,
        ) -> Handle {
            self.elements
                .borrow_mut()
                .push((name.local.to_string(), self.current_byte.get()));
            self.rcdom.create_element(name, attrs, flags)
        }

        fn create_comment(&self, text: StrTendril) -> Handle {
            self.rcdom.create_comment(text)
        }

        fn create_pi(&self, target: StrTendril, content: StrTendril) -> Handle {
            self.rcdom.create_pi(target, content)
        }

        fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
            self.rcdom.append(parent, child)
        }

        fn append_before_sibling(&self, sibling: &Handle, child: NodeOrText<Handle>) {
            self.rcdom.append_before_sibling(sibling, child)
        }

        fn append_based_on_parent_node(
            &self,
            element: &Handle,
            prev_element: &Handle,
            child: NodeOrText<Handle>,
        ) {
            self.rcdom
                .append_based_on_parent_node(element, prev_element, child)
        }

        fn append_doctype_to_document(
            &self,
            name: StrTendril,
            public_id: StrTendril,
            system_id: StrTendril,
        ) {
            self.rcdom
                .append_doctype_to_document(name, public_id, system_id);
        }

        fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
            self.rcdom.add_attrs_if_missing(target, attrs);
        }

        fn remove_from_parent(&self, target: &Handle) {
            self.rcdom.remove_from_parent(target);
        }

        fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
            self.rcdom.reparent_children(node, new_parent);
        }

        fn mark_script_already_started(&self, target: &Handle) {
            self.rcdom.mark_script_already_started(target);
        }

        fn set_current_line(&self, line_number: u64) {
            self.rcdom.set_current_line(line_number);
        }

        fn set_current_byte(&self, byte_offset: u64) {
            self.current_byte.set(byte_offset);
        }
    }

    fn parse(input: &str) -> ByteCapturingDOM {
        let sink = ByteCapturingDOM::new();
        driver::parse_document(sink, Default::default()).one(StrTendril::from(input))
    }

    #[test]
    fn element_byte_offsets_match_source_positions() {
        // <p>   starts at byte 0
        // <div> starts at byte 14  ("<p>hello</p>" = 12 chars + 2 for "</p>")
        //   <p>hello</p> = 12 bytes, </p> = 4 bytes → <div> at 16? Let's be precise:
        // "<p>hello</p><div>world</div>"
        //  0123456789012345678901234567
        //  <p> = 0, </p> = 8, <div> = 12
        let result = parse("<p>hello</p><div>world</div>");
        let elems = result.content_elements();

        assert_eq!(elems.len(), 2, "expected p and div, got: {:?}", elems);
        assert_eq!(elems[0], ("p".to_string(), 0));
        assert_eq!(elems[1], ("div".to_string(), 12));
    }

    #[test]
    fn nested_element_byte_offset() {
        // "<div><span>x</span></div>"
        //  01234567890123456789...
        // <div> = 0, <span> = 5
        let result = parse("<div><span>x</span></div>");
        let elems = result.content_elements();

        assert_eq!(elems.len(), 2, "expected div and span, got: {:?}", elems);
        assert_eq!(elems[0], ("div".to_string(), 0));
        assert_eq!(elems[1], ("span".to_string(), 5));
    }

    #[test]
    fn multibyte_content_does_not_shift_subsequent_offsets() {
        // "<p>café</p><span>next</span>"
        // 'é' = 2 bytes, so:
        // <p>    = byte 0
        // </p>   = byte 3+5 = byte 8  ("café" = c(1)+a(1)+f(1)+é(2) = 5 bytes)
        // <span> = byte 8 + 4 = byte 12 ("</p>" = 4 bytes)
        let result = parse("<p>café</p><span>next</span>");
        let elems = result.content_elements();

        assert_eq!(elems.len(), 2, "expected p and span, got: {:?}", elems);
        assert_eq!(elems[0], ("p".to_string(), 0));
        assert_eq!(elems[1], ("span".to_string(), 12));
    }
}
