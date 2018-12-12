#[macro_use]
extern crate criterion;
extern crate html5ever;

use std::fs;
use std::path::PathBuf;

use criterion::{Criterion, black_box, ParameterizedBenchmark};

use html5ever::tokenizer::{BufferQueue, TokenSink, Token, Tokenizer, TokenizerOpts, TokenSinkResult};
use html5ever::tendril::*;

struct Sink;

impl TokenSink for Sink {
    type Handle = ();

    fn process_token(&mut self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        // Don't use the token, but make sure we don't get
        // optimized out entirely.
        black_box(token);
        TokenSinkResult::Continue
    }
}

impl Sink {
    fn run(input: Vec<StrTendril>, opts: TokenizerOpts) {
        let mut tok = Tokenizer::new(Sink, opts.clone());
        let mut buffer = BufferQueue::new();
        for buf in input.into_iter() {
            buffer.push_back(buf);
            let _ = tok.feed(&mut buffer);
        }
        let _ = tok.feed(&mut buffer);
        tok.end();
    }
}

fn run_bench(c: &mut Criterion, name: &str, opts: TokenizerOpts) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("data/bench/");
    path.push(name);
    let mut file = fs::File::open(&path).ok().expect("can't open file");

    // Read the file and treat it as an infinitely repeating sequence of characters.
    let mut file_input = ByteTendril::new();
    file.read_to_tendril(&mut file_input).ok().expect("can't read file");
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

    let mut test_name = String::new();
    test_name.push_str("tokenizing");
    test_name.push_str(" ");
    test_name.push_str(name);

    c.bench_function(&test_name, move |b| b.iter(|| {
        let mut tok = Tokenizer::new(Sink, opts.clone());
        let mut buffer = BufferQueue::new();
        // We are doing clone inside the bench function, this is not ideal, but possibly
        // necessary since our iterator consumes the underlying buffer.
        for buf in input.clone().into_iter() {
            buffer.push_back(buf);
            let _ = tok.feed(&mut buffer);
        }
        let _ = tok.feed(&mut buffer);
        tok.end();
    }));
}



fn html5ever_benchmark(c: &mut Criterion) {
    run_bench(c, "lipsum.html", Default::default());
    run_bench(c, "lipsum-zh.html", Default::default());
    run_bench(c, "medium-fragment.html", Default::default());
    run_bench(c, "small-fragment.html", Default::default());
    run_bench(c, "tiny-fragment.html", Default::default());
    run_bench(c, "strong.html", Default::default());
}

criterion_group!(benches, html5ever_benchmark);
criterion_main!(benches);