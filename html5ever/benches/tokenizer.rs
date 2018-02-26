// Copyright 2014-2017  The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate rustc_test as test;
extern crate html5ever;

use std::{fs, env, cmp};
use std::path::PathBuf;
use std::default::Default;

use test::{black_box, Bencher, TestDesc, TestDescAndFn};
use test::{DynTestName, DynBenchFn, TDynBenchFn};

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

// This could almost be the TokenSink too, but it's not
// mut within run().
struct Bench {
    input: Vec<StrTendril>,
    clone_only: bool,
    opts: TokenizerOpts,
}

/// All tendrils in Bench.input are owned.
unsafe impl Send for Bench {}

impl Bench {
    fn new(name: &str, size: Option<usize>, clone_only: bool,
           opts: TokenizerOpts) -> Bench {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("data/bench/");
        path.push(name);
        let mut file = fs::File::open(&path).ok().expect("can't open file");

        // Read the file and treat it as an infinitely repeating sequence of characters.
        let mut file_input = ByteTendril::new();
        file.read_to_tendril(&mut file_input).ok().expect("can't read file");
        let file_input: StrTendril = file_input.try_reinterpret().unwrap();
        let size = size.unwrap_or(file_input.len());
        let mut stream = file_input.chars().cycle();

        // Break the input into chunks of 1024 chars (= a few kB).
        // This simulates reading from the network.
        let mut input = vec![];
        let mut total = 0usize;
        while total < size {
            // The by_ref() call is important, otherwise we get wrong results!
            // See rust-lang/rust#18045.
            let sz = cmp::min(1024, size - total);
            input.push(stream.by_ref().take(sz).collect::<String>().to_tendril());
            total += sz;
        }

        Bench {
            input: input,
            clone_only: clone_only,
            opts: opts,
        }
    }
}

impl TDynBenchFn for Bench {
    fn run(&self, bh: &mut Bencher) {
        bh.iter(|| {
            let input = self.input.clone();
            if self.clone_only {
                // Because the tokenizer consumes its buffers, we need
                // to clone inside iter().  We can benchmark this
                // separately and subtract it out.
                //
                // See rust-lang/rust#18043.
                black_box(input);
            } else {
                let mut tok = Tokenizer::new(Sink, self.opts.clone());
                let mut buffer = BufferQueue::new();
                for buf in input.into_iter() {
                    buffer.push_back(buf);
                    let _ = tok.feed(&mut buffer);
                }
                let _ = tok.feed(&mut buffer);
                tok.end();
            }
        });
    }
}

fn make_bench(name: &str, size: Option<usize>, clone_only: bool,
              opts: TokenizerOpts) -> TestDescAndFn {
    TestDescAndFn {
        desc: TestDesc::new(DynTestName([
            "tokenize ".to_string(),
            name.to_string(),
            size.map_or("".to_string(), |s| format!(" size {:7}", s)),
            (if clone_only { " (clone only)" } else { "" }).to_string(),
            (if opts.exact_errors { " (exact errors)" } else { "" }).to_string(),
        ].concat().to_string())),
        testfn: DynBenchFn(Box::new(Bench::new(name, size, clone_only, opts))),
    }
}

fn tests() -> Vec<TestDescAndFn> {
    let mut tests = vec!(make_bench("lipsum.html", Some(1024*1024), true, Default::default()));

    let mut opts_vec = vec!(Default::default());
    if env::var("BENCH_EXACT_ERRORS").is_ok() {
        opts_vec.push(TokenizerOpts {
            exact_errors: true,
            .. Default::default()
        });
    }

    for opts in opts_vec.iter() {
        for &file in ["lipsum.html", "lipsum-zh.html", "strong.html"].iter() {
            for &sz in [1024, 1024*1024].iter() {
                tests.push(make_bench(file, Some(sz), false, opts.clone()));
            }
        }

        for &file in ["tiny-fragment.html", "small-fragment.html", "medium-fragment.html"].iter() {
            tests.push(make_bench(file, None, false, opts.clone()));
        }

        if env::var("BENCH_UNCOMMITTED").is_ok() {
            // Not checked into the repo, so don't include by default.
            for &file in ["sina.com.cn.html", "wikipedia.html"].iter() {
                let name = format!("uncommitted/{}", file);
                tests.push(make_bench(&name, None, false, opts.clone()));
            }
        }
    }
    tests
}

fn main() {
    let args: Vec<_> = env::args().collect();
    test::test_main(&args, tests());
}
