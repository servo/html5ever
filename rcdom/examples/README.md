# How to Run the Examples

In order to run the examples, you should clone the repo and its submodules:

```bash
git clone https://github.com/servo/html5ever.git
pushd ./html5ever/
git submodule update --init
```

Then run the examples, using the just name of the subdirectory that holds the examples you want to run.

For example:

```bash
# builds and runs ./rcdom/examples/
cargo run --example print-rcdom
```

This is a common convention among many projects.
