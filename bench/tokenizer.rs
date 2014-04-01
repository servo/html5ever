/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::{io, os, str};

use test::{black_box, BenchHarness, TestDesc, TestDescAndFn};
use test::{DynTestName, DynBenchFn, TDynBenchFn};

use html5::tokenizer::{TokenSink, Token, Tokenizer};

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
    input: ~str,
    clone_only: bool,
}

impl Bench {
    fn new(name: &'static str, size: Option<uint>, clone_only: bool) -> Bench {
        let mut path = os::self_exe_path().expect("can't get exe path");
        path.push("../data/bench/");
        path.push(name);
        let mut file = io::File::open(&path).ok().expect("can't open file");
        let file_input = file.read_to_str().ok().expect("can't read file");

        let input = match size {
            None => file_input,
            Some(size) => {
                // Replicate the input in memory up to the desired size.
                let mut input = str::with_capacity(size);
                while input.len() < size {
                    input.push_str(file_input);
                }
                input.truncate(size);
                input
            }
        };

        Bench {
            input: input,
            clone_only: clone_only,
        }
    }
}

impl TDynBenchFn for Bench {
    fn run(&self, bh: &mut BenchHarness) {
        bh.iter(|| {
            let input = self.input.clone();
            if self.clone_only {
                // Because the tokenizer consumes its buffers, we need
                // to clone inside iter().  We can benchmark this
                // separately and subtract it out.
                black_box(input);
            } else {
                let mut sink = Sink;
                let mut tok = Tokenizer::new(&mut sink);
                tok.feed(self.input.clone());
                tok.end();
            }
        });
    }
}

fn make_bench(name: &'static str, size: Option<uint>, clone_only: bool) -> TestDescAndFn {
    TestDescAndFn {
        desc: TestDesc {
            name: DynTestName([
                ~"tokenize ",
                name.to_owned(),
                size.map_or(~"", |s| format!(" size {:7u}", s)),
                if clone_only { ~" (clone only)" } else { ~"" }
            ].concat()),
            ignore: false,
            should_fail: false,
        },
        testfn: DynBenchFn(~Bench::new(name, size, clone_only)),
    }
}

pub fn tests() -> ~[TestDescAndFn] {
    ~[
        make_bench("lipsum.html", Some(1024), true),
        make_bench("lipsum.html", Some(1024), false),
        make_bench("lipsum.html", Some(1024*1024), true),
        make_bench("lipsum.html", Some(1024*1024), false),
        make_bench("strong.html", Some(1024*1024), false),
        make_bench("strong.html", Some(1024), false),
        //make_bench("webapps.html", None, false),
    ]
}
