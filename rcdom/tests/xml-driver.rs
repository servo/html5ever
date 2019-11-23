use markup5ever_rcdom::{RcDom, SerializableHandle};
use xml5ever::driver;
use xml5ever::serialize;
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
    let document: SerializableHandle = dom.document.clone().into();
    serialize::serialize(&mut serialized, &document, Default::default()).unwrap();

    let dom_from_text = driver::parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .one(text.as_bytes());

    let mut reserialized = Vec::new();
    let document: SerializableHandle = dom_from_text.document.clone().into();
    serialize::serialize(&mut reserialized, &document, Default::default()).unwrap();

    assert_eq!(
        String::from_utf8(serialized).unwrap(),
        String::from_utf8(reserialized).unwrap()
    );
}

fn assert_serialization(text: &'static str, dom: RcDom) {
    let mut serialized = Vec::new();
    let document: SerializableHandle = dom.document.clone().into();
    serialize::serialize(&mut serialized, &document, Default::default()).unwrap();
    assert_eq!(String::from_utf8(serialized).unwrap(), text);
}
