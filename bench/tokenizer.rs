// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::old_io as io;
use std::{env, cmp};
use std::default::Default;
use std::vec::IntoIter;

use test::{black_box, Bencher, TestDesc, TestDescAndFn};
use test::{DynTestName, DynBenchFn, TDynBenchFn};
use test::ShouldFail::No;

use html5ever::tokenizer::{TokenSink, Token, Tokenizer, TokenizerOpts};

struct Sink;

impl TokenSink for Sink {
    fn process_token(&mut self, token: Token) {
        // Don't use the token, but make sure we don't get
        // optimized out entirely.
        black_box(token);
    }
}

// This could almost be the TokenSink too, but it's not
// mut within run().
struct Bench {
    input: Vec<String>,
    clone_only: bool,
    opts: TokenizerOpts,
}

impl Bench {
    fn new(name: &str, size: Option<usize>, clone_only: bool,
           opts: TokenizerOpts) -> Bench {
        let mut path = env::current_exe().ok().expect("can't get exe path");
        path.push("../data/bench/");
        path.push(name);
        let mut file = io::File::open(&path).ok().expect("can't open file");

        // Read the file and treat it as an infinitely repeating sequence of characters.
        let file_input = file.read_to_string().ok().expect("can't read file");
        let size = size.unwrap_or(file_input.len());
        let mut stream = file_input.as_slice().chars().cycle();

        // Break the input into chunks of 1024 chars (= a few kB).
        // This simulates reading from the network.
        let mut input = vec![];
        let mut total = 0us;
        while total < size {
            // The by_ref() call is important, otherwise we get wrong results!
            // See rust-lang/rust#18045.
            let sz = cmp::min(1024, size - total);
            input.push(stream.by_ref().take(sz).collect());
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
                for buf in input.into_iter() {
                    tok.feed(buf);
                }
                tok.end();
            }
        });
    }
}

fn make_bench(name: &str, size: Option<usize>, clone_only: bool,
              opts: TokenizerOpts) -> TestDescAndFn {
    TestDescAndFn {
        desc: TestDesc {
            name: DynTestName([
                "tokenize ".to_string(),
                name.to_string(),
                size.map_or("".to_string(), |s| format!(" size {:7}", s)),
                (if clone_only { " (clone only)" } else { "" }).to_string(),
                (if opts.exact_errors { " (exact errors)" } else { "" }).to_string(),
            ].concat().to_string()),
            ignore: false,
            should_fail: No,
        },
        testfn: DynBenchFn(box Bench::new(name, size, clone_only, opts)),
    }
}

pub fn tests() -> IntoIter<TestDescAndFn> {
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
                tests.push(make_bench(name.as_slice(), None, false, opts.clone()));
            }
        }
    }

    tests.into_iter()
}
