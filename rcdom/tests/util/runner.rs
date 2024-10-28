// Copyright 2024 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use libtest_mimic::{Arguments, Trial};

/// Simple container for storing tests for later execution
pub struct Test {
    pub name: String,
    pub skip: bool,
    pub test: Box<dyn Fn() + Send + Sync>,
}

impl Test {
    /// Invoke the stored test function
    ///
    /// A status message is printed if the wrapped closure completes
    /// or is marked as skipped. The test should panic to report
    /// failure.
    pub fn run(&self) {
        print!("test {} ...", self.name);
        if self.skip {
            println!(" SKIPPED");
        } else {
            (self.test)();
            println!(" ok");
        }
    }
}

pub fn run_all(tests: Vec<Test>) {
    let mut harness_tests = Vec::new();

    for test in tests {
        let harness_test = Trial::test(test.name.clone(), move || {
            test.run();
            Ok(())
        });
        harness_tests.push(harness_test);
    }
    let args = Arguments::from_args();
    libtest_mimic::run(&args, harness_tests).exit();
}
