#!/bin/bash
# Copyright 2014-2017 The html5ever Project Developers. See the
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
    cargo bench --all
    cargo test -p html5ever --features "rustc-test/capture"
    cargo test -p xml5ever --features "rustc-test/capture"
else
    cargo bench --all
    cargo test --all
fi

cargo doc --all
