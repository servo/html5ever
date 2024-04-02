// Copyright 2024 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// Simple container for storing tests for later execution
pub struct Test {
    pub name: String,
    pub skip: bool,
    pub test: Box<dyn Fn()>,
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
