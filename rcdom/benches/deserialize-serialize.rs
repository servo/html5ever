#[macro_use]
extern crate criterion;
extern crate html5ever;

use std::path::PathBuf;
use std::{fs, io};

use criterion::{BatchSize, Criterion};

use html5ever::{parse_document, tendril::*, ParseOpts};
use markup5ever_rcdom::{RcDom, SerializableHandle};

fn bench_deserialize_with_path(criterion: &mut Criterion, path: &PathBuf) {
    let test_name = format!("deserialize {:?}", path.file_name().unwrap());
    let data = fs::read(path).expect("Could not read test file");

    criterion.bench_function(&test_name, move |bencher| {
        bencher.iter_batched(
            || (),
            |_| {
                std::hint::black_box(deserialize_to_rcdom(&data));
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_serialize_with_path(criterion: &mut Criterion, path: &PathBuf) {
    let test_name = format!("serialize {:?}", path.file_name().unwrap());
    let data = fs::read(path).expect("Could not read test file");
    let dom = deserialize_to_rcdom(&data);

    criterion.bench_function(&test_name, move |bencher| {
        bencher.iter_batched(
            || Vec::with_capacity(data.len() * 2),
            |mut output| {
                serialize_rcdom(&dom, &mut output);
                std::hint::black_box(output);
            },
            BatchSize::SmallInput,
        )
    });
}

fn deserialize_to_rcdom(stream: &[u8]) -> RcDom {
    parse_document(RcDom::default(), ParseOpts::default())
        .from_utf8()
        .read_from(&mut io::Cursor::new(stream))
        .expect("Failed to parse document")
}

fn serialize_rcdom(dom: &RcDom, out: &mut Vec<u8>) {
    let document: SerializableHandle = dom.document.clone().into();
    html5ever::serialize(out, &document, Default::default()).expect("Failed to serialize document");
}

fn paths() -> Vec<PathBuf> {
    let mut base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base_path.pop();

    let make_path = |name| {
        base_path
            .join("html5ever")
            .join("data")
            .join("bench")
            .join(name)
    };

    vec![
        make_path("lipsum.html"),
        make_path("lipsum-zh.html"),
        make_path("medium-fragment.html"),
        make_path("small-fragment.html"),
        make_path("tiny-fragment.html"),
        make_path("strong.html"),
    ]
}

fn bench_deserialize(criterion: &mut Criterion) {
    for path in paths() {
        bench_deserialize_with_path(criterion, &path);
    }
}

fn bench_serialize(criterion: &mut Criterion) {
    for path in paths() {
        bench_serialize_with_path(criterion, &path);
    }
}

criterion_group!(benches, bench_deserialize, bench_serialize);
criterion_main!(benches);
