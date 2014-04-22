/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::{io, os, str};
use std::default::Default;

use test::{black_box, BenchHarness, TestDesc, TestDescAndFn};
use test::{DynTestName, DynBenchFn, TDynBenchFn};

use hubbub::hubbub;

// This could almost be the TokenSink too, but it's not
// mut within run().
struct Bench {
    input: ~str,
}

impl Bench {
    fn new(name: &str, size: Option<uint>) -> Bench {
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
                input
            }
        };

        Bench {
            input: input,
        }
    }
}

impl TDynBenchFn for Bench {
    fn run(&self, bh: &mut BenchHarness) {
        bh.iter(|| {
            let mut parser = hubbub::Parser("UTF-8", false);
            parser.enable_scripting(true);
            parser.enable_styling(true);
            parser.parse_chunk(self.input.as_bytes());
        });
    }
}

fn make_bench(name: &str, size: Option<uint>) -> TestDescAndFn {
    TestDescAndFn {
        desc: TestDesc {
            name: DynTestName([
                ~"tokenize ",
                name.to_owned(),
                size.map_or(~"", |s| format!(" size {:7u}", s)),
            ].concat()),
            ignore: false,
            should_fail: false,
        },
        testfn: DynBenchFn(~Bench::new(name, size)),
    }
}

pub fn tests() -> Vec<TestDescAndFn> {
    let mut tests = Vec::new();

    for &file in ["lipsum.html", "lipsum-zh.html", "strong.html"].iter() {
        for &sz in [1024, 1024*1024].iter() {
            tests.push(make_bench(file, Some(sz)));
        }
    }

    for &file in ["small-fragment.html", "medium-fragment.html"].iter() {
        tests.push(make_bench(file, None));
    }

    if os::getenv("BENCH_UNCOMMITTED").is_some() {
        // Not checked into the repo, so don't include by default.
        for &file in ["webapps.html", "sina.com.cn.html", "wikipedia.html"].iter() {
            let name: ~str = (~"uncommitted/").append(file);
            tests.push(make_bench(name.as_slice(), None));
        }
    }

    tests
}
