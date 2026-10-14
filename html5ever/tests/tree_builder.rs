use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode};
use html5ever::tendril::TendrilSink;
use html5ever::{
    parse_document, tree_builder::TreeSink, Attribute, ExpandedName, ParseOpts, QualName,
};
use markup5ever::tendril::StrTendril;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// A sink that builds no tree, and counts how often the tree builder asks it for an element's
/// name or compares two nodes. The tree builder does both for each element it looks at in the
/// stack of open elements.
#[derive(Default)]
struct Sink {
    next_id: Cell<usize>,
    names: RefCell<HashMap<usize, &'static QualName>>,
    lookups: Cell<usize>,
}

impl TreeSink for Sink {
    type Handle = usize;
    type Output = usize;
    type ElemName<'a> = ExpandedName<'a>;
    fn finish(self) -> usize {
        self.lookups.get()
    }

    fn get_document(&self) -> usize {
        0
    }

    fn get_template_contents(&self, target: &usize) -> usize {
        target + 1
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        self.lookups.set(self.lookups.get() + 1);
        x == y
    }

    fn elem_name(&self, target: &usize) -> ExpandedName<'_> {
        self.lookups.set(self.lookups.get() + 1);
        self.names
            .borrow()
            .get(target)
            .expect("not an element")
            .expanded()
    }

    fn create_element(&self, name: QualName, _: Vec<Attribute>, _: ElementFlags) -> usize {
        let id = self.next_id.get() + 2;
        self.next_id.set(id);
        // Leaked, to keep the sink small.
        self.names
            .borrow_mut()
            .insert(id, Box::leak(Box::new(name)));
        id
    }

    fn create_comment(&self, _text: StrTendril) -> usize {
        let id = self.next_id.get() + 2;
        self.next_id.set(id);
        id
    }

    fn create_pi(&self, _target: StrTendril, _value: StrTendril) -> usize {
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
    fn add_attrs_if_missing(&self, _target: &usize, _attrs: Vec<Attribute>) {}
    fn remove_from_parent(&self, _target: &usize) {}
    fn reparent_children(&self, _node: &usize, _new_parent: &usize) {}
    fn mark_script_already_started(&self, _node: &usize) {}
}

/// How many elements the tree builder looks at, per token, while parsing `html`.
fn lookups_per_token(html: &str, tokens: usize) -> usize {
    let lookups = parse_document(Sink::default(), ParseOpts::default()).one(html);
    lookups / tokens
}

/// Deeply nested markup, followed by tokens that each make the tree builder look for something
/// in the stack of open elements. Each shape is `(prefix, nested, tokens)`: `nested` and then
/// `tokens` are repeated `n` times.
const SHAPES: &[(&str, &str, &str)] = &[
    // https://github.com/servo/html5ever/issues/788
    ("", "<div>", "</div>"),
    ("", "<ul><li>", ""),
    ("", "<b>", ""),
    // Has an element in scope.
    ("", "<div>", "</li>"),
    ("", "<div>", "</h1>"),
    ("", "<div>", "</p>"),
    ("", "<div>", "</button>"),
    ("", "<div>", "</tr>"),
    ("<p><table>", "<div>", ""),
    ("<!DOCTYPE html><p><button>", "<div>", ""),
    // The other steps that look down the stack.
    ("", "<span>", "</x>"),
    ("", "<div>", "<li></li>"),
    ("", "<div>", "<table></table>"),
    ("", "<div>", "</option>"),
    ("<form>", "<div>", "<input>"),
    ("", "<rt>", "</body>"),
    ("<b>", "<div>", "<br>"),
    ("<b>", "<div>", "</b>"),
    ("<svg>", "<g>", "</x>"),
];

#[test]
fn stack_lookups_do_not_grow_with_depth() {
    // Walking the stack would look at about n / 2 elements per token.
    let n = 2000;
    for &(prefix, nested, tokens) in SHAPES {
        let html = format!("{prefix}{}{}", nested.repeat(n), tokens.repeat(n));
        let lookups = lookups_per_token(&html, 2 * n);
        assert!(
            lookups < 20,
            "{prefix}{nested}…{tokens}…: {lookups} lookups per token"
        );
    }
}
