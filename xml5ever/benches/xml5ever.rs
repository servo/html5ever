#[macro_use]
extern crate criterion;
extern crate markup5ever;
extern crate xml5ever;

use std::fs;
use std::path::PathBuf;

use criterion::Criterion;

use markup5ever::buffer_queue::BufferQueue;
use xml5ever::tendril::*;
use xml5ever::tokenizer::{ProcessResult, Token, TokenSink, XmlTokenizer};

struct Sink;

impl TokenSink for Sink {
    type Handle = ();

    fn process_token(&self, token: Token) -> ProcessResult<()> {
        // Don't use the token, but make sure we don't get
        // optimized out entirely.
        std::hint::black_box(token);
        ProcessResult::Continue
    }
}

fn run_bench(c: &mut Criterion, name: &str) {
    let mut path = PathBuf::from("./");
    path.push("data/bench/");
    path.push(name);
    let mut file = fs::File::open(&path).expect("can't open file");

    // Read the file and treat it as an infinitely repeating sequence of characters.
    let mut file_input = ByteTendril::new();
    file.read_to_tendril(&mut file_input)
        .expect("can't read file");
    let file_input: StrTendril = file_input.try_reinterpret().unwrap();
    let size = file_input.len();
    let mut stream = file_input.chars().cycle();

    // Break the input into chunks of 1024 chars (= a few kB).
    // This simulates reading from the network.
    let mut input = vec![];
    let mut total = 0usize;
    while total < size {
        // The by_ref() call is important, otherwise we get wrong results!
        // See rust-lang/rust#18045.
        let sz = std::cmp::min(1024, size - total);
        input.push(stream.by_ref().take(sz).collect::<String>().to_tendril());
        total += sz;
    }

    let test_name = format!("xml tokenizing {name}");

    c.bench_function(&test_name, move |b| {
        b.iter(|| {
            let tok = XmlTokenizer::new(Sink, Default::default());
            let buffer = BufferQueue::default();
            // We are doing clone inside the bench function, this is not ideal, but possibly
            // necessary since our iterator consumes the underlying buffer.
            for buf in input.clone().into_iter() {
                buffer.push_back(buf);
                let _ = tok.feed(&buffer);
            }
            let _ = tok.feed(&buffer);
            tok.end();
        })
    });
}

fn xml5ever_benchmarks(c: &mut Criterion) {
    run_bench(c, "strong.xml");
}

criterion_group!(benches, xml5ever_benchmarks);
criterion_main!(benches);
