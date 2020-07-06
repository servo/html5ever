#![no_main]
use libfuzzer_sys::fuzz_target;

use std::io::BufReader;
use html5ever::driver::ParseOpts;
use markup5ever_rcdom::{RcDom, SerializableHandle};
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{parse_document, serialize};

// Target inspired by the Rust-Fuzz project
// https://github.com/rust-fuzz/targets
fuzz_target!(|data: &[u8]| {
    let opts = ParseOpts {
        tree_builder: TreeBuilderOpts {
            drop_doctype: true,
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
