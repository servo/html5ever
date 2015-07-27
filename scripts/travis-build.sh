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

# Test without unstable first, to make sure src/tree_builder/rules.expanded.rs is up-to-date.
cargo test --no-run
cargo test | ./scripts/shrink-test-output.py
r=${PIPESTATUS[0]}
if [ $r -ne 0 ]; then exit $r; fi

if [ $TRAVIS_RUST_VERSION = nightly ]
then
    cargo test --no-run --features unstable
    cargo test --features unstable | ./scripts/shrink-test-output.py
    r=${PIPESTATUS[0]}
    if [ $r -ne 0 ]; then exit $r; fi
fi

cargo test --manifest-path capi/Cargo.toml

cargo doc
