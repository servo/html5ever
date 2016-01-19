# xml5ever

[![Build Status](https://travis-ci.org/Ygg01/xml5ever.svg?branch=master)](https://travis-ci.org/Ygg01/xml5ever)![http://www.apache.org/licenses/LICENSE-2.0](https://img.shields.io/badge/license-Apache-blue.svg)![https://opensource.org/licenses/MIT](https://img.shields.io/badge/license-MIT-blue.svg)

[API documentation](https://Ygg01.github.io/docs/xml5ever/xml5ever/index.html)

This crate provides a push based XML parser library that trades well-formedness for error recovery.

xml5ever is based largely on [html5ever](https://github.com/servo/html5ever) parser, so if you have experience with html5ever you will be familiar with xml5ever.

The library is dual licensed under MIT and Apache license.

#Why you should use xml5ever

Main use case for this library is when XML is badly formatted, usually from bad XML
templates. XML5 tries to handle most common errors, in a manner similar to HTML5.

## When you should use it?

  - You aren't interested in well-formed documents.
  - You need to get some info from your data even if it has errors (although not all possible errors are handled).
  - You want to use some advanced features like character references or xml namespaces.

## When you shouldn't use it

  - You need to have your document validated.
  - You require DTD support.

#Installation

Add xml5ever as a dependency in your project manifest.

```toml
    [dependencies]
    xml5ever = "0.1.1"
```

And add crate declaration in your lib.rs

```rust
    extern crate xml5ever
```

#Getting started

xml5ever is meant to be used as a push based parser, that ca
of software. Here are some examples, what can be done with xml5ever.

