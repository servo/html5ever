use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode};
use html5ever::tendril::TendrilSink;
use html5ever::{
    driver, expanded_name, local_name, ns, tree_builder::TreeSink, Attribute, ExpandedName,
    ParseOpts, QualName,
};
use markup5ever::tendril::StrTendril;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

#[derive(Default)]
struct Sink {
    next_id: Cell<usize>,
    names: RefCell<HashMap<usize, &'static QualName>>,
}

impl Sink {
    fn get_id(&self) -> usize {
        let id = self.next_id.get();
        self.next_id.set(id + 2);
        id
    }
}

impl TreeSink for Sink {
    type Handle = usize;
    type Output = Self;
    type ElemName<'a> = ExpandedName<'a>;
    fn finish(self) -> Self {
        self
    }

    fn get_document(&self) -> usize {
        0
    }

    fn get_template_contents(&self, target: &usize) -> usize {
        if let Some(expanded_name!(html "template")) =
            self.names.borrow().get(target).map(|n| n.expanded())
        {
            target + 1
        } else {
            panic!("not a template element")
        }
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }

    fn elem_name(&self, target: &usize) -> ExpandedName<'_> {
        self.names
            .borrow()
            .get(target)
            .expect("not an element")
            .expanded()
    }

    fn create_element(&self, name: QualName, _: Vec<Attribute>, _: ElementFlags) -> usize {
        let id = self.get_id();
        // N.B. We intentionally leak memory here to minimize the implementation complexity
        //      of this example code. A real implementation would either want to use a real
        //      real DOM tree implentation, or else use an arena as the backing store for
        //      memory used by the parser.
        self.names
            .borrow_mut()
            .insert(id, Box::leak(Box::new(name)));
        id
    }

    fn create_comment(&self, _text: StrTendril) -> usize {
        self.get_id()
    }

    #[allow(unused_variables)]
    fn create_pi(&self, target: StrTendril, value: StrTendril) -> usize {
        unimplemented!()
    }

    fn append_before_sibling(&self, _sibling: &usize, _new_node: NodeOrText<usize>) {}

    fn append_based_on_parent_node(
        &self,
        _element: &usize,
        _prev_element: &usize,
        _new_node: NodeOrText<usize>,
    ) {
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}
    fn set_quirks_mode(&self, _mode: QuirksMode) {}
    fn append(&self, _parent: &usize, _child: NodeOrText<usize>) {}

    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}
    fn add_attrs_if_missing(&self, target: &usize, _attrs: Vec<Attribute>) {
        assert!(self.names.borrow().contains_key(target), "not an element");
    }
    fn remove_from_parent(&self, _target: &usize) {}
    fn reparent_children(&self, _node: &usize, _new_parent: &usize) {}
    fn mark_script_already_started(&self, _node: &usize) {}

    fn clone_subtree(&self, _node: &Self::Handle) -> Self::Handle {
        // For this noop example, just return a new placeholder ID
        self.get_id()
    }
}

#[test]
fn test_driver_interrupted_by_non_script() {
    // https://github.com/servo/html5ever/issues/716
    let test_case = "<meta charset=\"UTF-8\" /><meta charset=\"UTF-8\" /> other stuff";
    let mut parser = driver::parse_document(Sink::default(), ParseOpts::default());
    parser.process(test_case.into());
    parser.finish();
}
