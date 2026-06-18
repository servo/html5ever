//! Integration tests for the `source-positions` feature.
//!
//! Verifies that byte offsets flow correctly from `BufferQueue` through the
//! tokenizer and tree builder all the way into `TreeSink::set_current_byte`,
//! and that the offsets correspond to the actual positions of element opening
//! tags in the source string.
//!
//! 2 Critical behaviours are under test:
//!
//! 1. When no explicit <head>,<html>,<body> tags are part of the payload
//!    they get injected implicitly, they should not skew the byte offset.
//! 2. When the above tags are explicitly part of the payload, they should be part
//!    of the count.

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
    /// element created.
    ///
    /// These are then later used for assertions.
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

        fn content_elements(&self) -> Vec<(String, u64)> {
            self.elements.borrow().clone()
        }
    }

    impl TreeSink for ByteCapturingDOM {
        type Handle = Handle;
        type Output = Self;

        type ElemName<'a> = ExpandedName<'a>;

        fn finish(self) -> Self {
            self
        }

        fn parse_error(&self, msg: Cow<'static, str>) {
            self.rcdom.parse_error(msg);
        }

        fn get_document(&self) -> Handle {
            self.rcdom.get_document()
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

        fn get_template_contents(&self, target: &Handle) -> Handle {
            self.rcdom.get_template_contents(target)
        }

        fn same_node(&self, x: &Handle, y: &Handle) -> bool {
            self.rcdom.same_node(x, y)
        }

        fn set_quirks_mode(&self, mode: QuirksMode) {
            self.rcdom.set_quirks_mode(mode)
        }

        fn append_before_sibling(&self, sibling: &Handle, child: NodeOrText<Handle>) {
            self.rcdom.append_before_sibling(sibling, child)
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
        let result = parse("<p>hello</p><div>world</div>");
        let elems = result.content_elements();

        assert_eq!(
            elems.len(),
            5,
            "expected html, head, body, p and div, got: {:?}",
            elems
        );
        assert_eq!(elems[0], ("html".to_string(), 0));
        assert_eq!(elems[1], ("head".to_string(), 0));
        assert_eq!(elems[2], ("body".to_string(), 0));
        assert_eq!(elems[3], ("p".to_string(), 0));
        assert_eq!(elems[4], ("div".to_string(), 12));
    }

    #[test]
    fn nested_element_byte_offset() {
        let result = parse("<div><span>x</span></div>");
        let elems = result.content_elements();

        assert_eq!(
            elems.len(),
            5,
            "expected html, head, body, div and span, got: {:?}",
            elems
        );
        assert_eq!(elems[0], ("html".to_string(), 0));
        assert_eq!(elems[1], ("head".to_string(), 0));
        assert_eq!(elems[2], ("body".to_string(), 0));
        assert_eq!(elems[3], ("div".to_string(), 0));
        assert_eq!(elems[4], ("span".to_string(), 5));
    }

    #[test]
    fn explicit_html_head_body_offsets() {
        let result = parse("<html><head></head><body><p>hi</p></body></html>");
        let elems = result.content_elements();

        assert_eq!(
            elems.len(),
            4,
            "expected html, head, body, p, got: {:?}",
            elems
        );
        assert_eq!(elems[0], ("html".to_string(), 0));
        assert_eq!(elems[1], ("head".to_string(), 6));
        assert_eq!(elems[2], ("body".to_string(), 19));
        assert_eq!(elems[3], ("p".to_string(), 25));
    }

    #[test]
    /// <span> should start at byte 12, and not 13 due to é being 2 bytes.
    fn multibyte_content_does_not_shift_subsequent_offsets() {
        let result = parse("<p>café</p><span>next</span>");
        let elems = result.content_elements();

        assert_eq!(
            elems.len(),
            5,
            "expected html, head, body, p and span, got: {:?}",
            elems
        );
        assert_eq!(elems[0], ("html".to_string(), 0));
        assert_eq!(elems[1], ("head".to_string(), 0));
        assert_eq!(elems[2], ("body".to_string(), 0));
        assert_eq!(elems[3], ("p".to_string(), 0));
        assert_eq!(elems[4], ("span".to_string(), 12));
    }
}
