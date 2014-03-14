#!/bin/bash -xe

mkdir -p build
${RUSTC-rustc} $RUSTFLAGS --out-dir build --lib html5.rs
${RUSTC-rustc} $RUSTFLAGS --out-dir build -L build examples/test_tokenizer.rs
