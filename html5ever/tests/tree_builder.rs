use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{
    local_name, ns, parse_document, parse_fragment, Attribute, ExpandedName, QualName,
};
use std::borrow::Cow;
use std::cell::RefCell;

/// A sink that doesn't build a tree. It logs the elements that are popped off the stack of open
/// elements, and the options that the tree builder asks it to clone into a selectedcontent.
struct Sink {
    /// The name of the element that each handle refers to, or `None` for other nodes.
    names: RefCell<Vec<Option<&'static QualName>>>,
    log: RefCell<Vec<String>>,
}

impl Sink {
    fn new() -> Self {
        Sink {
            // The document.
            names: RefCell::new(vec![None]),
            log: RefCell::default(),
        }
    }

    fn new_node(&self, name: Option<QualName>) -> usize {
        let mut names = self.names.borrow_mut();
        // Leak the name so that `elem_name` can return a reference to it.
        names.push(name.map(|name| &*Box::leak(Box::new(name))));
        names.len() - 1
    }

    fn log(&self, event: &str, element: &usize) {
        let local_name = self.elem_name(element).local.to_string();
        self.log.borrow_mut().push(format!("{event} {local_name}"));
    }
}

impl TreeSink for Sink {
    type Handle = usize;
    type Output = String;
    type ElemName<'a> = ExpandedName<'a>;

    fn finish(self) -> String {
        self.log.into_inner().join(", ")
    }

    fn get_document(&self) -> usize {
        0
    }

    fn elem_name<'a>(&'a self, target: &'a usize) -> ExpandedName<'a> {
        self.names.borrow()[*target]
            .expect("not an element")
            .expanded()
    }

    fn create_element(&self, name: QualName, _: Vec<Attribute>, flags: ElementFlags) -> usize {
        let element = self.new_node(Some(name));
        if flags.template {
            // The template contents get the next handle.
            self.new_node(None);
        }
        element
    }

    fn create_comment(&self, _text: StrTendril) -> usize {
        self.new_node(None)
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> usize {
        self.new_node(None)
    }

    fn get_template_contents(&self, target: &usize) -> usize {
        target + 1
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }

    fn pop(&self, node: &usize) {
        self.log("pop", node);
    }

    fn maybe_clone_an_option_into_selectedcontent(&self, option: &usize) {
        self.log("maybe clone", option);
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}
    fn set_quirks_mode(&self, _mode: QuirksMode) {}
    fn append(&self, _parent: &usize, _child: NodeOrText<usize>) {}
    fn append_before_sibling(&self, _sibling: &usize, _new_node: NodeOrText<usize>) {}
    fn append_based_on_parent_node(&self, _: &usize, _: &usize, _: NodeOrText<usize>) {}
    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}
    fn add_attrs_if_missing(&self, _target: &usize, _attrs: Vec<Attribute>) {}
    fn remove_from_parent(&self, _target: &usize) {}
    fn reparent_children(&self, _node: &usize, _new_parent: &usize) {}
}

/// Parse `input` as the contents of a `<body>` element, and return what the sink logged.
fn parse_body_fragment(input: &str) -> String {
    let context = QualName::new(None, ns!(html), local_name!("body"));
    parse_fragment(Sink::new(), Default::default(), context, vec![], false).one(input)
}

#[test]
fn every_element_that_leaves_the_stack_of_open_elements_is_popped() {
    // An end tag that pops elements until it pops its own element.
    assert_eq!(
        parse_body_fragment("<div><span></div>"),
        "pop span, pop div, pop html"
    );
    // Clearing the stack back to a table context.
    assert_eq!(
        parse_body_fragment("<table><b><tr>"),
        "pop b, pop tr, pop tbody, pop table, pop html"
    );
    // The adoption agency algorithm, without a furthest block.
    assert_eq!(parse_body_fragment("<b><i></b>"), "pop i, pop b, pop html");
    // The adoption agency algorithm, removing the span between the formatting element and the
    // furthest block from the stack.
    assert_eq!(
        parse_body_fragment("<b><span><div></b>"),
        "pop span, pop b, pop b, pop div, pop html"
    );
    // "Any other end tag" in the "in body" insertion mode.
    assert_eq!(
        parse_body_fragment("<span><em></span>"),
        "pop em, pop span, pop html"
    );
    // An end tag in foreign content.
    assert_eq!(
        parse_body_fragment("<svg><g></g></svg>"),
        "pop g, pop svg, pop html"
    );
    // A frameset start tag, which only does anything in a document.
    assert_eq!(
        parse_document(Sink::new(), Default::default()).one("<p><frameset>"),
        "pop head, pop p, pop body, pop frameset, pop html"
    );
}

/// <https://github.com/servo/html5ever/issues/712>
#[test]
fn maybe_clone_an_option_into_selectedcontent_runs_for_every_popped_option() {
    assert_eq!(
        parse_body_fragment("<select><option></option></select>"),
        "pop option, maybe clone option, pop select, pop html"
    );
    // Options closed by an option, an optgroup, an hr and a select end tag, and at the end of the
    // input.
    assert_eq!(
        parse_body_fragment(
            "<select><option><option><optgroup><option><hr><option></select><option>"
        ),
        "pop option, maybe clone option, \
         pop option, maybe clone option, \
         pop option, maybe clone option, pop optgroup, \
         pop option, maybe clone option, pop select, \
         pop option, maybe clone option, pop html"
    );
    // An SVG element named "option" isn't an option element.
    assert_eq!(
        parse_body_fragment("<svg><option></option></svg>"),
        "pop option, pop svg, pop html"
    );
}
