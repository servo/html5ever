#Examples

The examples have been designed with [`cargo-script`](https://github.com/DanielKeep/cargo-script) in mind.

Here I'll just give broad overview how to install [`cargo script`] for Rust 1.5. For more details, check out [cargo-script repository](https://github.com/DanielKeep/cargo-script).

    cargo install cargo-script
    
#Executing examples

All examples expect some form of input. If you fail to provide it, the program will **wait indefinitely for input** .
What this means, is that you have to pass a file to your cargo script command. For that reason there is a tiny `example.xml` file.

To run the examples you: 

```bash
  cd examples
  cargo script xml_tokenizer.rs < example.xml
```

Or you can pass some xml your typed

```bash
  cd examples
  cargo script xml_tokenizer.rs <<< "<xml>This is <b>my</b> XML</xml>"
```
