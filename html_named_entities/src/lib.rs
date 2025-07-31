// Copyright 2014-2025 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implements a tokenizer for named character references in HTML.
//!
//! A full list of all entities can be found on
//! [w3c.org](https://dev.w3.org/html5/spec-LC/named-character-references.html).

#![deny(missing_docs)]

mod codegen;
mod interface;
mod tokenizer;

pub use interface::{CharRef, InputSource};
pub use tokenizer::{
    format_name_error, NamedReferenceTokenizationResult, NamedReferenceTokenizerState,
};
