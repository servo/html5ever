use markup5ever::{expanded_name, local_name, ns};
use markup5ever_rcdom::{serialize_xml, NodeData, RcDom};
use xml5ever::driver;
use xml5ever::tendril::TendrilSink;

#[test]
fn el_ns_serialize() {
    assert_eq_serialization(
        "<a:title xmlns:a=\"http://www.foo.org/\" value=\"test\">Test</a:title>",
        driver::parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one("<a:title xmlns:a=\"http://www.foo.org/\" value=\"test\">Test</title>".as_bytes()),
    );
}

#[test]
fn nested_ns_serialize() {
    assert_eq_serialization("<a:x xmlns:a=\"http://www.foo.org/\" xmlns:b=\"http://www.bar.org/\" value=\"test\"><b:y/></a:x>",
        driver::parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one("<a:x xmlns:a=\"http://www.foo.org/\" xmlns:b=\"http://www.bar.org/\" value=\"test\"><b:y/></a:x>".as_bytes()));
}

#[test]
fn def_ns_serialize() {
    assert_eq_serialization(
        "<table xmlns=\"html4\"><td></td></table>",
        driver::parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one("<table xmlns=\"html4\"><td></td></table>".as_bytes()),
    );
}

#[test]
fn undefine_ns_serialize() {
    assert_eq_serialization(
        "<a:x xmlns:a=\"http://www.foo.org\"><a:y xmlns:a=\"\"><a:z/></a:y</a:x>",
        driver::parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one(
                "<a:x xmlns:a=\"http://www.foo.org\"><a:y xmlns:a=\"\"><a:z/></a:y</a:x>"
                    .as_bytes(),
            ),
    );
}

#[test]
fn redefine_default_ns_serialize() {
    assert_eq_serialization(
        "<x xmlns=\"http://www.foo.org\"><y xmlns=\"\"><z/></y</x>",
        driver::parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one("<x xmlns=\"http://www.foo.org\"><y xmlns=\"\"><z/></y</x>".as_bytes()),
    );
}

#[test]
fn attr_serialize() {
    assert_serialization(
        "<title value=\"test\">Test</title>",
        driver::parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one("<title value='test'>Test".as_bytes()),
    );
}

#[test]
fn from_utf8() {
    assert_serialization(
        "<title>Test</title>",
        driver::parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one("<title>Test".as_bytes()),
    );
}

fn assert_eq_serialization(text: &'static str, dom: RcDom) {
    let mut serialized = Vec::new();
    serialize_xml(&mut serialized, &dom.document).unwrap();

    let dom_from_text = driver::parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .one(text.as_bytes());

    let mut reserialized = Vec::new();
    serialize_xml(&mut reserialized, &dom_from_text.document).unwrap();

    assert_eq!(
        String::from_utf8(serialized).unwrap(),
        String::from_utf8(reserialized).unwrap()
    );
}

fn assert_serialization(text: &'static str, dom: RcDom) {
    let mut serialized = Vec::new();
    serialize_xml(&mut serialized, &dom.document).unwrap();
    assert_eq!(String::from_utf8(serialized).unwrap(), text);
}

#[test]
fn xmlns_on_root() {
    assert_serialization(
       r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:sodipodi="http://inkscape.sourceforge.net/DTD/sodipodi-0.dtd">
    <sodipodi:namedview>
        ...
    </sodipodi:namedview>

    <path d="..." sodipodi:nodetypes="cccc"/>
</svg>"##,
        driver::parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one(r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:sodipodi="http://inkscape.sourceforge.net/DTD/sodipodi-0.dtd">
    <sodipodi:namedview>
        ...
    </sodipodi:namedview>

    <path d="..." sodipodi:nodetypes="cccc"/>
</svg>"##.as_bytes()),
    );
}

#[test]
fn xmlns_applies_to_attr() {
    assert_serialization(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:sodipodi="http://inkscape.sourceforge.net/DTD/sodipodi-0.dtd">
    <path d="..." sodipodi:nodetypes="cccc"/>
</svg>"##,
        driver::parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one(r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:sodipodi="http://inkscape.sourceforge.net/DTD/sodipodi-0.dtd">
    <path d="..." sodipodi:nodetypes="cccc"/>
</svg>"##.as_bytes()),
    );
}

#[test]
fn template() {
    let dom = driver::parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .one(r##"<template xmlns="http://www.w3.org/1999/xhtml"/>"##.as_bytes());
    let contents = driver::parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .one(r##"<div xmlns="http://www.w3.org/1999/xhtml">Pass</div>"##.as_bytes());
    match &dom.document.children.borrow()[0].data {
        NodeData::Element {
            name,
            template_contents,
            ..
        } => {
            match name.expanded() {
                expanded_name!(html "template") => {},
                name => panic!("Root element should be template, was {:?}", name),
            }
            template_contents
                .borrow()
                .as_ref()
                .expect("Should have template contents")
                .children
                .borrow_mut()
                .push(contents.document.children.borrow()[0].clone());
        },
        _ => panic!("Unexpected child of document")
    }
    assert_serialization(
        r##"<template xmlns="http://www.w3.org/1999/xhtml"><div>Pass</div></template>"##,
        dom,
    );
}
