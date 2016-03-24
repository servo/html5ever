# html5ever

[![Build Status](https://travis-ci.org/servo/html5ever.svg?branch=master)](https://travis-ci.org/servo/html5ever)

[API Documentation][API documentation]

html5ever is an HTML parser developed as part of the [Servo](https://github.com/servo/servo) project.

It can parse and serialize HTML according to the [WHATWG](https://whatwg.org/) specs (aka "HTML5").  There are some omissions at present, most of which are documented [in the bug tracker](https://github.com/servo/html5ever/issues?q=is%3Aopen+is%3Aissue+label%3Aweb-compat).  html5ever passes all tokenizer tests from [html5lib-tests](https://github.com/html5lib/html5lib-tests), and most tree builder tests outside of the unimplemented features.  The goal is to pass all html5lib tests, and also provide all hooks needed by a production web browser, e.g. `document.write`.

Note that the HTML syntax is a language almost, but not quite, entirely unlike XML.  For correct parsing of XHTML, use an XML parser.  (That said, many XHTML documents in the wild are serialized in an HTML-compatible form.)

html5ever is written in [Rust](http://www.rust-lang.org/), so it avoids the most notorious security problems from C, but has performance similar to a parser written in C.  You can call html5ever as if it were a C library, without pulling in a garbage collector or other heavy runtime requirements.


## Getting started in Rust

Add html5ever as a dependency in your [`Cargo.toml`](http://crates.io/) file:

```toml
[dependencies]
html5ever = "*"
```

Then take a look at [`examples/print-rcdom.rs`](https://github.com/servo/html5ever/blob/master/examples/print-rcdom.rs) and the [API documentation][].

## Getting started in other languages

Bindings for Python and other languages are much desired.


## Working on html5ever

To fetch the test suite, you need to run

```
git submodule update --init
```

Run `cargo doc` in the repository root to build local documentation under `target/doc/`.


## Details

html5ever uses callbacks to manipulate the DOM, so it works with your choice of DOM representation.  A simple reference-counted DOM is included.

html5ever exclusively uses UTF-8 to represent strings.  In the future it will support other document encodings (and UCS-2 `document.write`) by converting input.

The code is cross-referenced with the WHATWG syntax spec, and eventually we will have a way to present code and spec side-by-side.

html5ever builds against the official stable releases of Rust, though some optimizations are only supported on nightly releases.

[API documentation]: http://doc.servo.org/html5ever/index.html
