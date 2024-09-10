use html5ever::driver;
use html5ever::tendril::stream::TendrilSink;
use html5ever::tendril::StrTendril;
use html5ever::ExpandedName;
use html5ever::QualName;
use markup5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use markup5ever::{local_name, namespace_url, ns, Attribute};
use markup5ever_rcdom::{Handle, RcDom};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};

pub struct LineCountingDOM {
    pub line_vec: RefCell<Vec<(QualName, u64)>>,
    pub current_line: Cell<u64>,
    pub rcdom: RcDom,
}

impl TreeSink for LineCountingDOM {
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

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        self.line_vec
            .borrow_mut()
            .push((name.clone(), self.current_line.get()));
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
        self.current_line.set(line_number);
    }
}

#[test]
fn check_four_lines() {
    // Input
    let sink = LineCountingDOM {
        line_vec: RefCell::new(vec![]),
        current_line: Cell::new(1),
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
    assert_eq!(*actual.line_vec.borrow(), expected);
}
