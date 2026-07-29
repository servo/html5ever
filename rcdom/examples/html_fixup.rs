// illustrates how to generate valid xhtml from html fragment with common errors

extern crate html5ever;
extern crate markup5ever_rcdom as rcdom;

use std::default::Default;

use html5ever::driver::ParseOpts;
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{parse_fragment, serialize, QualName};
use markup5ever::{local_name, namespace_url, ns};
use rcdom::{RcDom, SerializableHandle};

fn parse_and_serialize(input: StrTendril) -> StrTendril {
    let dom = parse_fragment(
        RcDom::default(),
        ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("body")),
        vec![],
    )
    .one(input);
    let inner: SerializableHandle = dom.document.children.borrow()[0].clone().into();

    let mut result = vec![];
    serialize(&mut result, &inner, Default::default()).unwrap();
    StrTendril::try_from_byte_slice(&result).unwrap()
}

fn main() {
    let sample_input = [
        "<P>hello</P  >   ",
        "http://example.com/page?query=1&foo=2",
        "http://example.com/page?query=1&amp;foo=2",
        "<p>foo",
    ];

    println!("\n-- normalize html and fix common errors --\n");
    sample_input.iter().for_each(|input_ref| {
        let input = *input_ref;
        let result = parse_and_serialize(input.into());
        println!("input:  {}", input);
        println!("output: {}\n", result);
    })
}
