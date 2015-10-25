#![crate_name="xml5ever"]
#![crate_type="dylib"]

#![cfg_attr(feature = "unstable", feature(plugin))]
#![cfg_attr(feature = "unstable", plugin(string_cache_plugin))]

#[macro_use] extern crate log;
#[macro_use] extern crate mac;
#[macro_use] extern crate string_cache;

extern crate rc;
extern crate phf;
extern crate tendril;
extern crate time;

macro_rules! time {
    ($e:expr) => {{
        let t0 = ::time::precise_time_ns();
        let result = $e;
        let dt = ::time::precise_time_ns() - t0;
        (result, dt)
    }}
}

#[macro_use] mod util;
pub mod tokenizer;
pub mod tree_builder;
pub mod rcdom;

use tokenizer::{XmlTokenizerOpts, XmlTokenizer, TokenSink};
use tree_builder::{TreeSink, XmlTreeBuilder};

/// Parse and send results to a `TreeSink`.
///
/// ## Example
///
/// ```ignore
/// let mut sink = MySink;
/// parse_xml_to(&mut sink, one_input(my_str), Default::default());
/// ```
pub fn parse_xml_to<
        Sink:TreeSink,
        It: IntoIterator<Item=tendril::StrTendril>
    >(
        sink: Sink,
        input: It,
        opts: XmlTokenizerOpts) -> Sink {

    let tb = XmlTreeBuilder::new(sink);
    let mut tok = XmlTokenizer::new(tb, opts);
    for s in input {
        tok.feed(s);
    }
    tok.end();
    tok.unwrap().unwrap()
}


/// Parse into a type which implements `ParseResult`.
///
/// ## Example
///
/// ```ignore
/// let dom: RcDom = parse_xml(one_input(my_str), Default::default());
/// ```
pub fn parse_xml<Output, It>(input: It, opts: XmlTokenizerOpts) -> Output
    where Output: ParseResult,
          It: IntoIterator<Item=tendril::StrTendril>,
{
    let sink = parse_xml_to(Default::default(), input, opts);
    ParseResult::get_result(sink)
}

/// Results which can be extracted from a `TreeSink`.
///
/// Implement this for your parse tree data type so that it
/// can be returned by `parse()`.
pub trait ParseResult {
    type Sink: TreeSink + Default;
    fn get_result(sink: Self::Sink) -> Self;
}

/// Tokenize and send results to a `XTokenSink`.
///
/// ## Example
///
/// ```ignore
/// let mut sink = MySink;
/// tokenize_xml_to(&mut sink, one_input(my_str), Default::default());
/// ```
pub fn tokenize_xml_to<
        Sink: TokenSink,
        It: IntoIterator<Item=tendril::StrTendril>
    >(
        sink: Sink,
        input: It,
        opts: XmlTokenizerOpts) -> Sink {

    let mut tok = XmlTokenizer::new(sink, opts);
    for s in input {
        tok.feed(s);
    }
    tok.end();
    tok.unwrap()
}
