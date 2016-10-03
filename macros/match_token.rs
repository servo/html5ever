// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!

Implements the `match_token!()` macro for use by the HTML tree builder
in `src/tree_builder/rules.rs`.


## Example

```rust
match_token!(token {
    CommentToken(text) => 1,

    tag @ <base> <link> <meta> => 2,

    </head> => 3,

    </body> </html> </br> => else,

    tag @ </_> => 4,

    token => 5,
})
```


## Syntax

Because of the simplistic parser, the macro invocation must
start with exactly `match_token!(token {` (with whitespace as specified)
and end with exactly `})`.

The left-hand side of each match arm is an optional `name @` binding, followed by

  - an ordinary Rust pattern that starts with an identifier or an underscore, or

  - a sequence of HTML tag names as identifiers, each inside "<...>" or "</...>"
    to match an open or close tag respectively, or

  - a "wildcard tag" "<_>" or "</_>" to match all open tags or all close tags
    respectively.

The right-hand side is either an expression or the keyword `else`.

Note that this syntax does not support guards or pattern alternation like
`Foo | Bar`.  This is not a fundamental limitation; it's done for implementation
simplicity.


## Semantics

Ordinary Rust patterns match as usual.  If present, the `name @` binding has
the usual meaning.

A sequence of named tags matches any of those tags.  A single sequence can
contain both open and close tags.  If present, the `name @` binding binds (by
move) the `Tag` struct, not the outer `Token`.  That is, a match arm like

```rust
tag @ <html> <head> => ...
```

expands to something like

```rust
TagToken(tag @ Tag { name: atom!("html"), kind: StartTag })
| TagToken(tag @ Tag { name: atom!("head"), kind: StartTag }) => ...
```

A wildcard tag matches any tag of the appropriate kind, *unless* it was
previously matched with an `else` right-hand side (more on this below).

The expansion of this macro reorders code somewhat, to satisfy various
restrictions arising from moves.  However it provides the semantics of in-order
matching, by enforcing the following restrictions on its input:

  - The last pattern must be a variable or the wildcard "_".  In other words
    it must match everything.

  - Otherwise, ordinary Rust patterns and specific-tag patterns cannot appear
    after wildcard tag patterns.

  - No tag name may appear more than once.

  - A wildcard tag pattern may not occur in the same arm as any other tag.
    "<_> <html> => ..." and "<_> </_> => ..." are both forbidden.

  - The right-hand side "else" may only appear with specific-tag patterns.
    It means that these specific tags should be handled by the last,
    catch-all case arm, rather than by any wildcard tag arm.  This situation
    is common in the HTML5 syntax.
*/

use std::collections::{HashMap, HashSet};
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Clone)]
struct Source<'a> {
    src: &'a str,
}

impl<'a> Source<'a> {
    fn consume(&mut self, n: usize) -> &'a str {
        let (before, after) = self.src.split_at(n);
        self.src = after;
        before
    }

    fn find(&mut self, s: &str) -> Option<&'a str> {
        self.src.find(s).map(|position| {
            let before = self.consume(position);
            self.consume(s.len());
            before
        })
    }

    fn consume_if_present(&mut self, s: &str) -> bool {
        let present = self.src.starts_with(s);
        if present {
            self.consume(s.len());
        }
        present
    }

    fn expect(&mut self, s: &str) {
        assert!(self.consume_if_present(s), "{:?}… does not start with {:?}", &self.src[..50], s);
    }

    /// Not exactly Rust whitespace, but close enough
    fn consume_whitespace(&mut self) {
        while self.src.starts_with(&[' ', '\t', '\n', '\r'][..]) {
            self.consume(1);
        }
    }

    /// Not exactly the syntax of a Rust identifier, but close enough
    fn consume_ident(&mut self) -> Option<&'a str> {
        let end = self.src.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(self.src.len());
        if end > 0 {
            Some(self.consume(end))
        } else {
            None
        }
    }

    fn find_top_level(&mut self, start_at: usize, delimeter: u8) -> usize {
        let mut i = start_at;
        let bytes = self.src.as_bytes();
        loop {
            let b = *bytes.get(i).expect("unbalanced brackets");
            i += 1;
            if b == delimeter {
                return i
            }
            match b {
                b'{' => i = self.find_top_level(i, b'}'),
                b'[' => i = self.find_top_level(i, b']'),
                b'(' => i = self.find_top_level(i, b')'),
                _ => {}
            }
        }
    }
}

pub fn expand_match_tokens(from: &Path, to: &Path) {
//    use std::fmt::Write;
    let mut source = String::new();
    File::open(from).unwrap().read_to_string(&mut source).unwrap();

    let mut source = Source { src: &*source };

    let mut file = File::create(to).unwrap();
    let mut write = |s: &str| file.write_all(s.as_bytes()).unwrap();
//    let mut write = |s: &str| ();
    while let Some(before) = source.find("match_token!") {
        write(before);
        source.expect("(token {");
        let mut arms = Vec::new();
        loop {
            source.consume_whitespace();
            if source.consume_if_present("})") {
                break
            }
            arms.push(parse_arm(&mut source));
        }
//        write!(file, "{:#?}\n", arms);
        write_match_token(arms, &mut write);
    }
    write(source.src);
}

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
enum TagKind {
    StartTag,
    EndTag,
}

/// A single tag, as may appear in an LHS.
///
/// `name` is `None` for wildcards.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
struct Tag {
    kind: TagKind,
    name: Option<String>,
}

/// Left-hand side of a pattern-match arm.
#[derive(Debug)]
enum LHS {
    Pattern(String),
    Tags(Vec<Tag>),
}

/// Right-hand side of a pattern-match arm.
#[derive(Debug)]
enum RHS {
    Else,
    Expression(String),
}

/// A whole arm, including optional outer `name @` binding.
#[derive(Debug)]
struct Arm {
    binding: Option<String>,
    lhs: LHS,
    rhs: RHS,
}

fn parse_arm(source: &mut Source) -> Arm {
    loop {
        source.consume_whitespace();
        if source.consume_if_present("//") {
            source.find("\n");
        } else {
            break
        }
    }
    let start = source.clone();
    let mut binding = None;
    if let Some(ident) = source.consume_ident() {
        source.consume_whitespace();
        if source.consume_if_present("@") {
            binding = Some(ident.to_owned())
        } else {
            *source = start
        }
    }

    Arm {
        binding: binding,
        lhs: parse_lhs(source),
        rhs: parse_rhs(source),
    }
}

fn parse_lhs(source: &mut Source) -> LHS {
    source.consume_whitespace();
    if source.consume_if_present("<") {
        let mut tags = Vec::new();
        loop {
            tags.push(Tag {
                kind: if source.consume_if_present("/") {
                    TagKind::EndTag
                } else {
                    TagKind::StartTag
                },
                name: if source.consume_if_present("_") {
                    None
                } else {
                    Some(source.consume_ident().expect("expected identifier (tag name)").to_owned())
                }
            });
            assert!(source.consume_if_present(">"), "expected '>' closing a tag pattern");
            source.consume_whitespace();
            if !source.consume_if_present("<") {
                break
            }
        }
        source.consume_whitespace();
        assert!(source.consume_if_present("=>"));
        LHS::Tags(tags)
    } else {
        LHS::Pattern(source.find("=>").expect("did not find =>").to_owned())
    }
}

fn parse_rhs(source: &mut Source) -> RHS {
    source.consume_whitespace();
    let start_at;
    let delimeter;
    if source.consume_if_present("else,") {
        return RHS::Else
    } else if source.src.starts_with("{") {
        start_at = 1;
        delimeter = b'}';
    } else {
        start_at = 0;
        delimeter = b',';
    }
    let end = source.find_top_level(start_at, delimeter);
    let expr = source.consume(end);
    if delimeter == b'}' {
        source.consume_whitespace();
        source.consume_if_present(",");
    }
    RHS::Expression(expr.to_owned())
}

/// Description of a wildcard match arm.
///
/// We defer generating code for these until we process the last, catch-all
/// arm.  This isn't part of the AST produced by `parse()`; it's created
/// while processing that AST.
struct WildcardArm {
    binding: String,
    kind: TagKind,
    expr: String,
}

fn write_match_token<F>(mut arms: Vec<Arm>, write: &mut F) where F: FnMut(&str) {
    write("match token {\n");

    // Handle the last arm specially at the end.
    let last_arm = arms.pop().unwrap();

    // Tags we've seen, used for detecting duplicates.
    let mut seen_tags: HashSet<Tag> = HashSet::new();

    // Case arms for wildcard matching.  We collect these and
    // emit them later.
    let mut wildcards: Vec<WildcardArm> = Vec::new();

    // Tags excluded (by an 'else' RHS) from wildcard matching.
    let mut wild_excluded: HashMap<TagKind, Vec<Tag>> = HashMap::new();

    for Arm { binding, lhs, rhs } in arms {
        // Build Rust syntax for the `name @` binding, if any.
        let binding = match binding {
            Some(ident) => format!("{} @ ", ident),
            None => String::new(),
        };

        match (lhs, rhs) {
            (LHS::Pattern(_), RHS::Else) => panic!("'else' may not appear with an ordinary pattern"),

            // ordinary pattern => expression
            (LHS::Pattern(pat), RHS::Expression(expr)) => {
                if !wildcards.is_empty() {
                    panic!("ordinary patterns may not appear after wildcard tags {:?} {:?}", pat, expr);
                }
                write(&format!("    {}{} => {}\n", binding, pat, expr));
            }

            // <tag> <tag> ... => else
            (LHS::Tags(tags), RHS::Else) => {
                for tag in tags {
                    if !seen_tags.insert(tag.clone()) {
                        panic!("duplicate tag");
                    }
                    if tag.name.is_none() {
                        panic!("'else' may not appear with a wildcard tag");
                    }
                    wild_excluded.entry(tag.kind).or_insert_with(Vec::new).push(tag.clone());
                }
            }

            // <_> => expression
            // <tag> <tag> ... => expression
            (LHS::Tags(tags), RHS::Expression(expr)) => {
                // Is this arm a tag wildcard?
                // `None` if we haven't processed the first tag yet.
                let mut wildcard = None;
                for tag in tags {
                    if !seen_tags.insert(tag.clone()) {
                        panic!("duplicate tag");
                    }

                    match tag.name {
                        // <tag>
                        Some(_) => {
                            if !wildcards.is_empty() {
                                panic!("specific tags may not appear after wildcard tags");
                            }

                            if wildcard == Some(true) {
                                panic!("wildcard tags must appear alone");
                            }

                            if wildcard.is_some() {
                                // Push the delimeter `|` if it's not the first tag.
                                write(" |\n    ");
                            } else {
                                write("    ");
                            }
                            write(&make_tag_pattern(&binding, tag));

                            wildcard = Some(false);
                        }

                        // <_>
                        None => {
                            if wildcard.is_some() {
                                panic!("wildcard tags must appear alone");
                            }
                            wildcard = Some(true);
                            wildcards.push(WildcardArm {
                                binding: binding.clone(),
                                kind: tag.kind,
                                expr: expr.clone(),
                            });
                        }
                    }
                }

                match wildcard {
                    None => panic!("[internal macro error] tag arm with no tags"),
                    Some(false) => {
                        write(" =>\n    ");
                        write(&expr);
                        write("\n");
                    }
                    Some(true) => {} // codegen for wildcards is deferred
                }
            }
        }
    }

    // Time to process the last, catch-all arm.  We will generate something like
    //
    //     last_arm_token => {
    //         let enable_wildcards = match last_arm_token {
    //             TagToken(Tag { kind: EndTag, name: atom!("body"), .. }) => false,
    //             TagToken(Tag { kind: EndTag, name: atom!("html"), .. }) => false,
    //             // ...
    //             _ => true,
    //         };
    //
    //         match (enable_wildcards, last_arm_token) {
    //             (true, TagToken(name @ Tag { kind: StartTag, .. }))
    //                 => ...,  // wildcard action for start tags
    //
    //             (true, TagToken(name @ Tag { kind: EndTag, .. }))
    //                 => ...,  // wildcard action for end tags
    //
    //             (_, token) => ...  // using the pattern from that last arm
    //         }
    //     }

    let Arm { binding, lhs, rhs } = last_arm;

    let (last_pat, last_expr) = match (binding, lhs, rhs) {
        (Some(_), _, _) => panic!("the last arm cannot have an @-binding"),
        (None, LHS::Tags(_), _) => panic!("the last arm cannot have tag patterns"),
        (None, _, RHS::Else) => panic!("the last arm cannot use 'else'"),
        (None, LHS::Pattern(p), RHS::Expression(e)) => (p, e)
    };

    write("    last_arm_token => {\n");
    write("        let enable_wildcards = match last_arm_token {\n");

    // Code for the `false` arms inside `let enable_wildcards = ...`.
    for (_, tags) in wild_excluded {
        for tag in tags {
            write(&format!("            {} => false,\n", make_tag_pattern("", tag)));
        }
    }

    write("            _ => true,\n");
    write("        };\n");
    write("        match (enable_wildcards, last_arm_token) {\n");

    // Code for the wildcard actions.
    for WildcardArm { binding, kind, expr } in wildcards {
        let pat = make_tag_pattern(&binding, Tag { kind: kind, name: None });
        write(&format!("            (true, {}) =>\n", pat));
        write(&format!("                {}\n", expr));
    }

    write(&format!("            (_, {}) => {}\n", last_pat, last_expr));
    write("        }\n");
    write("    }\n");
    write("}\n");
}

fn make_tag_pattern(binding: &str, tag: Tag) -> String {
    let mut s = format!(
        "::tree_builder::types::TagToken({}::tokenizer::Tag {{ kind: {:?}, ",
        binding, tag.kind);
    if let Some(name) = tag.name {
        write!(s, "name: atom!({:?}), ", name).unwrap();
    }
    s.push_str(".. })");
    s
}
