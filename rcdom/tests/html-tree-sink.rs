use html5ever::driver;
use html5ever::tendril::stream::TendrilSink;
use html5ever::tendril::StrTendril;
use html5ever::ExpandedName;
use html5ever::QualName;
use markup5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use markup5ever::{local_name, namespace_url, ns, Attribute};
use markup5ever_rcdom::{Handle, RcDom};
use std::borrow::Cow;

pub struct LineCountingDOM {
    pub line_vec: Vec<(QualName, u64)>,
    pub current_line: u64,
    pub rcdom: RcDom,
}

impl TreeSink for LineCountingDOM {
    type Output = Self;

    fn finish(self) -> Self {
        self
    }

    type Handle = Handle;

    fn parse_error(&mut self, msg: Cow<'static, str>) {
        self.rcdom.parse_error(msg);
    }

    fn get_document(&mut self) -> Handle {
        self.rcdom.get_document()
    }

    fn get_template_contents(&mut self, target: &Handle) -> Handle {
        self.rcdom.get_template_contents(target)
    }

    fn set_quirks_mode(&mut self, mode: QuirksMode) {
        self.rcdom.set_quirks_mode(mode)
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        self.rcdom.same_node(x, y)
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> ExpandedName<'a> {
        self.rcdom.elem_name(target)
    }

    fn create_element(
        &mut self,
        name: QualName,
        attrs: Vec<Attribute>,
        flags: ElementFlags,
    ) -> Handle {
        self.line_vec.push((name.clone(), self.current_line));
        self.rcdom.create_element(name, attrs, flags)
    }

    fn create_comment(&mut self, text: StrTendril) -> Handle {
        self.rcdom.create_comment(text)
    }

    fn create_pi(&mut self, target: StrTendril, content: StrTendril) -> Handle {
        self.rcdom.create_pi(target, content)
    }

    fn append(&mut self, parent: &Handle, child: NodeOrText<Handle>) {
        self.rcdom.append(parent, child)
    }

    fn append_before_sibling(&mut self, sibling: &Handle, child: NodeOrText<Handle>) {
        self.rcdom.append_before_sibling(sibling, child)
    }

    fn append_based_on_parent_node(
        &mut self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        self.rcdom
            .append_based_on_parent_node(element, prev_element, child)
    }

    fn append_doctype_to_document(
        &mut self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        self.rcdom
            .append_doctype_to_document(name, public_id, system_id);
    }

    fn add_attrs_if_missing(&mut self, target: &Handle, attrs: Vec<Attribute>) {
        self.rcdom.add_attrs_if_missing(target, attrs);
    }

    fn remove_from_parent(&mut self, target: &Handle) {
        self.rcdom.remove_from_parent(target);
    }

    fn reparent_children(&mut self, node: &Handle, new_parent: &Handle) {
        self.rcdom.reparent_children(node, new_parent);
    }

    fn mark_script_already_started(&mut self, target: &Handle) {
        self.rcdom.mark_script_already_started(target);
    }

    fn set_current_line(&mut self, line_number: u64) {
        self.current_line = line_number;
    }
}

#[test]
fn check_four_lines() {
    // Input
    let sink = LineCountingDOM {
        line_vec: vec![],
        current_line: 1,
        rcdom: RcDom::default(),
    };
    let mut result_tok = driver::parse_document(sink, Default::default());
    result_tok.process(StrTendril::from("<a>\n"));
    result_tok.process(StrTendril::from("</a>\n"));
    result_tok.process(StrTendril::from("<b>\n"));
    result_tok.process(StrTendril::from("</b>"));
    // Actual Output
    let actual = result_tok.finish();
    // Expected Output
    let expected = vec![
        (QualName::new(None, ns!(html), local_name!("html")), 1),
        (QualName::new(None, ns!(html), local_name!("head")), 1),
        (QualName::new(None, ns!(html), local_name!("body")), 1),
        (QualName::new(None, ns!(html), local_name!("a")), 1),
        (QualName::new(None, ns!(html), local_name!("b")), 3),
    ];
    // Assertion
    assert_eq!(actual.line_vec, expected);
}
