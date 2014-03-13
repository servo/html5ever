/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[feature(macro_rules)];

pub mod tokenizer;

struct TokenPrinter;

impl tokenizer::TokenSink for TokenPrinter {
    fn process_token(&mut self, token: tokenizer::Token) {
        println!("{:?}", token);
    }
}

fn main() {
    let mut sink = TokenPrinter;
    let mut tok = tokenizer::Tokenizer::new(&mut sink);
    tok.feed("<div novalue unquoted=foo singlequoted='bar' doublequoted=\"baz\">Hello, world!</div>");
}
