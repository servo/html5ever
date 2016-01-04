# xml5ever

[![Build Status](https://travis-ci.org/Ygg01/xml5ever.svg?branch=master)](https://travis-ci.org/Ygg01/xml5ever)![http://www.apache.org/licenses/LICENSE-2.0](https://img.shields.io/badge/license-Apache-blue.svg)![https://opensource.org/licenses/MIT](https://img.shields.io/badge/license-MIT-blue.svg)

This crate provides a pull XML parser library that trades well-formedness for error recovery.

xml5ever is based largely on [html5ever](https://github.com/servo/html5ever) parser, so if you have experience with html5ever you will be familiar with html5ever.

The library is dual licensed under MIT and Apache license.

#Why you should use xml5ever

Main use case for this library is when XML is badly formatted, usually from bad XML
templates. XML5 tries to handle most common errors, in a manner similar to HTML5.

## When you should use it?

  - You aren't interested in well-formed documents.
  - You need to get some info from your data even if it has errors (although not all errors are handled).
  - You want to use fancy XML 1.1 features like character references.

## When you shouldn't use it

  - You need to have your document validated.
  - You require DTD support.

#Documentation

The API is fully located at [API documentation](https://Ygg01.github.io/docs/xml5ever/xml5ever/index.html)

#Installation

Add xml5ever as a dependency in your Cargo.toml file

```toml
    [dependencies]
    xml5ever = "0.1.0"
```
#Getting started

xml5ever is meant to be used as a library, so it isn't the most user friendly piece
of software. However, its still possible to create a toy pretty printer.

Note: Before we start in  examples I'll assume you are using [`cargo script`](https://github.com/DanielKeep/cargo-script) or making a separate crate (the examples
require `cargo script` or manually setting up `rustc` which is an exercise I leave to
the reader.

#Token printer

The basis of xml5ever is its tokenizer and tree builder. Roughly speaking tokenizer
takes input and returns a set of tokens like comment, processing instruction, start
tag, end tag, etc.

First let's define our dependencies:

```toml
    [dependencies]
    xml5ever = "0.1.0"
    tendril = "0.1.3"
```

With dependencies declared, we can now make a simple xml tokenizer. First step is to
define a [`TokenSink`](https://ygg01.github.io/docs/xml5ever/xml5ever/tokenizer/trait.TokenSink.html). [`TokenSink`](https://ygg01.github.io/docs/xml5ever/xml5ever/tokenizer/trait.TokenSink.html) are traits that received stream of [`Tokens`](https://ygg01.github.io/docs/xml5ever/xml5ever/tokenizer/enum.Token.html).

In our case we'll define a unit struct, without any fields.

```rust
    struct SimpleTokenPrinter;
```

To make `SimpleTokenPrinter` a [`TokenSink`](https://ygg01.github.io/docs/xml5ever/xml5ever/tokenizer/trait.TokenSink.html), we need to implement [process_token](https://ygg01.github.io/docs/xml5ever/xml5ever/tokenizer/trait.TokenSink.html#tymethod.process_token) method.

```rust
    impl TokenSink for SimpleTokenPrinter {
        fn process_token(&mut self, token: Token) {
            match token {
                CharacterTokens(b) => {
                    println!("TEXT: {}", &*b);
                },
                NullCharacterToken => print!("NULL"),
                TagToken(tag) => {
                    println!("{:?} {} ", tag.kind, &*tag.name.local);
                },
                ParseError(err) => {
                    println!("ERROR: {}", err);
                },
                PIToken(Pi{ref target, ref data}) => {
                    println!("PI : <?{} {}?>", &*target, &*data);
                },
                CommentToken(ref comment) => {
                    println!("<!--{:?}-->", &*comment);
                },
                EOFToken => {
                    println!("EOF");
                },
                DoctypeToken(Doctype{ref name, ref public_id, ..}) => {
                    println!("<!DOCTYPE {:?} {:?}>", &*name, &*public_id);
                }
            }
        }
    }
```

Now we need to actually use `SimpleTokenPrinter` to process some input. For input
we'll use `stdin`. However, xml5ever `tokenize_xml_to` method only takes `StrTendril`. So we need to construct a
[`ByteTendril`](http://doc.servo.org/tendril/type.ByteTendril.html) using `ByteTendril::new()`, then read the `stdin` using [`read_to_tendril`](http://doc.servo.org/tendril/trait.ReadExt.html#tymethod.read_to_tendril).

Once that is set, to make `SimpleTokenPrinter` parse the input, by calling,
`tokenize_xml_to` with it as the first parameter.

```rust
    fn main() {
        let sink = SimpleTokenPrinter;

        // We need a ByteTendril to read a file
        let mut input = ByteTendril::new();
        // Using SliceExt.read_to_tendril we read stdin
        io::stdin().read_to_tendril(&mut input).unwrap();
        // For xml5ever we need StrTendril, so we reinterpret it
        // into StrTendril.
        //
        // You might wonder, how does `try_reinterpret` know we
        // need StrTendril and the answer is type inference based
        // on `tokenize_xml_to` signature.
        let input = input.try_reinterpret().unwrap();

        tokenize_xml_to(sink, Some(input), XmlTokenizerOpts {
            profile: true,
            exact_errors: true,
            .. Default::default()
        });
    }
```

NOTE: `unwrap` causes panic, it's only OK to use in simple examples.

For full source code check out: [`examples/simple_xml_tokenizer.rs`](https://github.com/Ygg01/xml5ever/blob/master/examples/simple_xml_tokenizer.rs)

Once we have successfully compiled the example we run the example with inline
xml

```bash
    cargo script simple_xml_tokenizer.rs <<< "<xml>Text with <b>bold words</b>!</xml>"
```

or by sending an [`examples/example.xml`](https://github.com/Ygg01/xml5ever/blob/master/examples/simple_xml_tokenizer.rs) located in same folder as examples.

```bash
    cargo script simple_xml_tokenizer.rs < example.xml
```

#Tree printer

To actually get an XML document tree from the xml5ever, you need to use a `TreeSink`.
`TreeSink` is in many way similar to the TokenSink. Basically, TokenSink takes data
and returns list of tokens, while TreeSink takes tokens and returns a tree of parsed
XML document. Do note, that this is a simplified explanation and consult
documentation for more info.

Ok, with that in mind, let's build us a TreePrinter. For example if we get an XML
file like:

```xml
    <student>
        <first-name>Bobby</first-name>
        <last-name>Tables</last-name>
    </student>
```

We'd want a structure similar to this:

```
#document
    student
        first-name
            #text Bobby
        last-name
            #text Tables

```
We won't print anything other than element names and text fields. So comments,
doctypes and other such elements are ignored.

First part is similar to making SimpleTokenPrinter:

```rust
    // We need to allocate an input tendril for xml5ever
    let mut input = ByteTendril::new();
    // Using SliceExt.read_to_tendril functions we can read stdin
    io::stdin().read_to_tendril(&mut input).unwrap();
    let input = input.try_reinterpret().unwrap();
```

This time, we need an implementation of [`TreeSink`](https://ygg01.github.io/docs/xml5ever/xml5ever/tree_builder/interface/trait.TreeSink.html). xml5ever comes with a
built-in `TreeSink` implementation called [`RcDom`](https://ygg01.github.io/docs/xml5ever/xml5ever/rcdom/struct.RcDom.html). To process input into
a `TreeSink` we use the following line:

```rust
    let dom: RcDom = parse(one_input(input), Default::default());
```

Let's analyze it a bit. First there is `let dom: RcDom`. We need this part,
because the type inferencer can't infer which TreeSink implementation we mean
in this particular scenario.

Next is the [`parse`](https://ygg01.github.io/docs/xml5ever/xml5ever/fn.parse.html) function which takes an iterator of StrTendril and TreeBuilder
settings to produce a ParseResult.

Function [`one_input`](https://ygg01.github.io/docs/xml5ever/xml5ever/fn.one_input.html) is a convenience function that turns any value into an iterator. In this case
it converts a StrTendril into an Iterator over itself.

Ok, so now that we parsed our tree what with it? Well, for that we might need some
kind of function that will help us traverse it. We shall call that function `walk`.

```rust
    fn walk(prefix: &str, handle: Handle) {
        let node = handle.borrow();

        print!("{}", prefix);
        match node.node {
            Document
                => println!("#document"),

            Text(ref text)  => {
                println!("#text {}", escape_default(text))
            },

            Element(ref name, _) => {
                println!("{}", name.local);
            },

            _ => {},

        }

        let new_indent = {
            let mut temp = String::new();
            temp.push_str(prefix);
            temp.push_str("    ");
            temp
        };

        for child in node.children.iter()
            .filter(|child| match child.borrow().node {
                Text(_) | Element (_, _) => true,
                _ => false,
            }
        ) {
            walk(&new_indent, child.clone());
        }
    }
```

Function `walk` takes current text used for indentation and handle for current node.
Current text used is appended with more characters to illustrate current level of
indentation.

For simplicity we only displayed nodes of type `Document`, `Text` or `Element` nodes.
Similarly we filter to only iterate over Text or Element nodes (there can be only one document and since its the root element it can't be a children node).


For full source code check out: [`examples/xml_tree_printer.rs`](https://github.com/Ygg01/xml5ever/blob/master/examples/xml_tree_printer.rs)
