#![no_main]
use libfuzzer_sys::fuzz_target;

use markup5ever_rcdom::{RcDom, SerializableHandle};
use std::io::BufReader;
use xml5ever::driver::parse_document;
use xml5ever::driver::XmlParseOpts;
use xml5ever::serialize::serialize;
use xml5ever::tendril::TendrilSink;
use xml5ever::tree_builder::XmlTreeBuilderOpts;

// Target inspired by the Rust-Fuzz project
// https://github.com/rust-fuzz/targets
fuzz_target!(|data: &[u8]| {
    let opts = XmlParseOpts {
        tree_builder: XmlTreeBuilderOpts {
            ..Default::default()
        },
        ..Default::default()
    };

    let dom = parse_document(RcDom::default(), opts)
        .from_utf8()
        .read_from(&mut BufReader::new(data));

    let dom = if let Ok(dom) = dom {
        dom
    } else {
        return;
    };

    let mut out = std::io::sink();
    let document: SerializableHandle = dom.document.into();
    let _ = serialize(&mut out, &document, Default::default());
});
