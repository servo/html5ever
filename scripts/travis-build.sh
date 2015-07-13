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

cargo doc
cargo test --no-run
cargo test | ./scripts/shrink-test-output.py
r=${PIPESTATUS[0]}
if [ $r -ne 0 ]; then exit $r; fi

cargo test --manifest-path dom_sink/Cargo.toml --no-run
cargo test --manifest-path dom_sink/Cargo.toml | ./scripts/shrink-test-output.py
r=${PIPESTATUS[0]}
if [ $r -ne 0 ]; then exit $r; fi

cargo test --manifest-path capi/Cargo.toml
