// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use html5ever::driver::{self, ParseOpts};
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::{TreeBuilder, TreeBuilderOpts};
use html5ever::{local_name, ns, QualName};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

/// The names of the open elements, written like in the html5lib tree construction tests.
fn open_elements(tree_builder: &TreeBuilder<Handle, RcDom>) -> Vec<String> {
    tree_builder.with_open_elements(|elements| {
        elements
            .iter()
            .map(|element| {
                let NodeData::Element { ref name, .. } = element.data else {
                    panic!("not an element");
                };
                match name.ns {
                    ns!(html) => name.local.to_string(),
                    ns!(svg) => format!("svg {}", name.local),
                    ns!(mathml) => format!("math {}", name.local),
                    _ => panic!("unexpected namespace"),
                }
            })
            .collect()
    })
}

/// The text of the children of the current node.
fn current_node_text(tree_builder: &TreeBuilder<Handle, RcDom>) -> String {
    tree_builder.with_open_elements(|elements| {
        let current_node = elements.last().expect("no current node");
        let mut text = String::new();
        for child in current_node.children.borrow().iter() {
            if let NodeData::Text { ref contents } = child.data {
                text.push_str(&contents.borrow());
            }
        }
        text
    })
}

#[test]
fn open_elements_in_document() {
    let mut parser = driver::parse_document(RcDom::default(), ParseOpts::default());
    parser.process("<!DOCTYPE html>".into());
    assert!(open_elements(&parser.tokenizer.sink).is_empty());

    parser.process("<table><td><b>".into());
    assert_eq!(
        open_elements(&parser.tokenizer.sink),
        ["html", "body", "table", "tbody", "tr", "td", "b"]
    );

    parser.process("</table><svg><circle>".into());
    assert_eq!(
        open_elements(&parser.tokenizer.sink),
        ["html", "body", "svg svg", "svg circle"]
    );

    parser.tokenizer.end();
    assert!(open_elements(&parser.tokenizer.sink).is_empty());
}

#[test]
fn open_elements_in_fragment() {
    let context = QualName::new(None, ns!(html), local_name!("ul"));
    let mut parser = driver::parse_fragment(
        RcDom::default(),
        ParseOpts::default(),
        context,
        vec![],
        true,
    );
    assert_eq!(open_elements(&parser.tokenizer.sink), ["html"]);

    parser.process("<li><p>".into());
    assert_eq!(open_elements(&parser.tokenizer.sink), ["html", "li", "p"]);
}

#[test]
fn text_insertion_mode() {
    for name in [
        "iframe", "noembed", "noframes", "noscript", "script", "style", "textarea", "title", "xmp",
    ] {
        let mut parser = driver::parse_document(RcDom::default(), ParseOpts::default());
        parser.process("<body>".into());
        assert!(!parser.tokenizer.sink.is_in_text_insertion_mode());

        parser.process(format!("<{name}>").into());
        assert!(parser.tokenizer.sink.is_in_text_insertion_mode(), "{name}");
        assert_eq!(
            open_elements(&parser.tokenizer.sink),
            ["html", "body", name]
        );

        // Character tokens are inserted into the current node, which is that element.
        parser.process("<p>".into());
        assert!(parser.tokenizer.sink.is_in_text_insertion_mode(), "{name}");
        assert_eq!(current_node_text(&parser.tokenizer.sink), "<p>", "{name}");

        parser.process(format!("</{name}>").into());
        assert!(!parser.tokenizer.sink.is_in_text_insertion_mode(), "{name}");
        assert_eq!(open_elements(&parser.tokenizer.sink), ["html", "body"]);
    }
}

#[test]
fn text_insertion_mode_ends_at_eof() {
    let mut parser = driver::parse_document(RcDom::default(), ParseOpts::default());
    parser.process("<script>".into());
    assert!(parser.tokenizer.sink.is_in_text_insertion_mode());

    parser.tokenizer.end();
    assert!(!parser.tokenizer.sink.is_in_text_insertion_mode());
}

#[test]
fn not_text_insertion_mode() {
    // The tokenizer switches to the PLAINTEXT state, but the insertion mode stays "in body".
    let mut parser = driver::parse_document(RcDom::default(), ParseOpts::default());
    parser.process("<plaintext>".into());
    assert!(!parser.tokenizer.sink.is_in_text_insertion_mode());

    // Character tokens in a table are handled in the "in table text" insertion mode.
    let mut parser = driver::parse_document(RcDom::default(), ParseOpts::default());
    parser.process("<table>x".into());
    assert!(!parser.tokenizer.sink.is_in_text_insertion_mode());

    // Foreign content.
    let mut parser = driver::parse_document(RcDom::default(), ParseOpts::default());
    parser.process("<svg><script>".into());
    assert!(!parser.tokenizer.sink.is_in_text_insertion_mode());
    parser.process("</script><style>".into());
    assert!(!parser.tokenizer.sink.is_in_text_insertion_mode());

    // Without scripting, the contents of `noscript` are parsed as markup.
    let opts = ParseOpts {
        tree_builder: TreeBuilderOpts {
            scripting_enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut parser = driver::parse_document(RcDom::default(), opts);
    parser.process("<body><noscript>".into());
    assert!(!parser.tokenizer.sink.is_in_text_insertion_mode());

    // The tokenizer starts in the script data state, but the text is inserted into the root
    // element in the "in body" insertion mode.
    let context = QualName::new(None, ns!(html), local_name!("script"));
    let mut parser = driver::parse_fragment(
        RcDom::default(),
        ParseOpts::default(),
        context,
        vec![],
        true,
    );
    parser.process("<p>".into());
    assert!(!parser.tokenizer.sink.is_in_text_insertion_mode());
    assert_eq!(open_elements(&parser.tokenizer.sink), ["html"]);
    assert_eq!(current_node_text(&parser.tokenizer.sink), "<p>");
}
