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
    tok.feed("<div>Hello, world!</div>");
}
