#!/bin/bash
# Copyright 2015 The html5ever Project Developers. See the
# COPYRIGHT file at the top-level directory of this distribution.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

set -ex

if [ $TRAVIS_RUST_VERSION = nightly ]
then
    cargo test --features "rustc-test/capture"
    cargo test --features "rustc-test/capture unstable"
    (cd macros && cargo build)
else
    cargo test
fi

cargo doc
