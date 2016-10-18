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

use quote::{ToTokens, Tokens};
use self::visit::{Visitor, RecursiveVisitor};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::mem;
use std::path::Path;
use std::slice;
use syn;

mod visit;

pub fn expand_match_tokens(from: &Path, to: &Path) {
    let mut source = String::new();
    File::open(from).unwrap().read_to_string(&mut source).unwrap();
    let mut crate_ = syn::parse_crate(&source).expect("Parsing rules.rs module");
    RecursiveVisitor { node_visitor: ExpanderVisitor }.visit_crate(&mut crate_);
    let mut tokens = Tokens::new();
    crate_.to_tokens(&mut tokens);
    let code = tokens.to_string().replace("{ ", "{\n").replace(" }", "\n}");
    File::create(to).unwrap().write_all(code.as_bytes()).unwrap();
}

struct ExpanderVisitor;

impl Visitor for ExpanderVisitor {
    fn visit_expression(&mut self, expr: &mut syn::Expr) {
        let tts;
        if let syn::Expr::Mac(ref mut macro_) = *expr {
            if macro_.path == syn::Path::from("match_token") {
                tts = mem::replace(&mut macro_.tts, Vec::new());
            } else {
                return
            }
        } else {
            return
        }
        let (to_be_matched, arms) = parse_match_token_macro(tts);
        let tokens = expand_match_token_macro(to_be_matched, arms);
        *expr = syn::parse_expr(&tokens.to_string()).expect("Parsing a match expression");
    }
}

fn parse_match_token_macro(tts: Vec<syn::TokenTree>) -> (syn::Ident, Vec<Arm>) {
    use syn::TokenTree::Delimited;
    use syn::DelimToken::{Brace, Paren};

    let mut tts = tts.into_iter();
    let inner_tts = if let Some(Delimited(syn::Delimited { delim: Paren, tts })) = tts.next() {
        tts
    } else {
        panic!("expected one top-level () block")
    };
    assert_eq!(tts.len(), 0);

    let mut tts = inner_tts.into_iter();
    let ident = if let Some(syn::TokenTree::Token(syn::Token::Ident(ident))) = tts.next() {
        ident
    } else {
        panic!("expected ident")
    };

    let block = if let Some(Delimited(syn::Delimited { delim: Brace, tts })) = tts.next() {
        tts
    } else {
        panic!("expected one {} block")
    };
    assert_eq!(tts.len(), 0);

    let mut tts = block.iter();
    let mut arms = Vec::new();
    while tts.len() > 0 {
        arms.push(parse_arm(&mut tts))
    }
    (ident, arms)
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
    name: Option<syn::Ident>,
}

/// Left-hand side of a pattern-match arm.
#[derive(Debug)]
enum LHS {
    Pattern(Tokens),
    Tags(Vec<Tag>),
}

/// Right-hand side of a pattern-match arm.
#[derive(Debug)]
enum RHS {
    Else,
    Expression(Tokens),
}

/// A whole arm, including optional outer `name @` binding.
#[derive(Debug)]
struct Arm {
    binding: Option<syn::Ident>,
    lhs: LHS,
    rhs: RHS,
}

fn parse_arm(tts: &mut slice::Iter<syn::TokenTree>) -> Arm {
    Arm {
        binding: parse_binding(tts),
        lhs: parse_lhs(tts),
        rhs: parse_rhs(tts),
    }
}

fn parse_binding(tts: &mut slice::Iter<syn::TokenTree>) -> Option<syn::Ident> {
    let start = tts.clone();
    if let (Some(&syn::TokenTree::Token(syn::Token::Ident(ref ident))),
            Some(&syn::TokenTree::Token(syn::Token::At))) = (tts.next(), tts.next()) {
        Some(ident.clone())
    } else {
        *tts = start;
        None
    }
}

fn consume_if_present(tts: &mut slice::Iter<syn::TokenTree>, expected: syn::Token) -> bool {
    if let Some(&syn::TokenTree::Token(ref first)) = tts.as_slice().first() {
        if *first == expected {
            tts.next();
            return true
        }
    }
    false
}

fn parse_lhs(tts: &mut slice::Iter<syn::TokenTree>) -> LHS {
    if consume_if_present(tts, syn::Token::Lt) {
        let mut tags = Vec::new();
        loop {
            tags.push(Tag {
                kind: if consume_if_present(tts, syn::Token::BinOp(syn::BinOpToken::Slash)) {
                    TagKind::EndTag
                } else {
                    TagKind::StartTag
                },
                name: if consume_if_present(tts, syn::Token::Underscore) {
                    None
                } else {
                    if let Some(&syn::TokenTree::Token(syn::Token::Ident(ref ident))) = tts.next() {
                        Some(ident.clone())
                    } else {
                        panic!("expected identifier (tag name)")
                    }
                }
            });
            assert!(consume_if_present(tts, syn::Token::Gt), "expected '>' closing a tag pattern");
            if !consume_if_present(tts, syn::Token::Lt) {
                break
            }
        }
        assert!(consume_if_present(tts, syn::Token::FatArrow));
        LHS::Tags(tags)
    } else {
        let mut pattern = Tokens::new();
        for tt in tts {
            if let &syn::TokenTree::Token(syn::Token::FatArrow) = tt {
                return LHS::Pattern(pattern)
            }
            tt.to_tokens(&mut pattern)
        }
        panic!("did not find =>")
    }
}

fn parse_rhs(tts: &mut slice::Iter<syn::TokenTree>) -> RHS {
    use syn::DelimToken::Brace;
    let start = tts.clone();
    let first = tts.next();
    let after_first = tts.clone();
    let second = tts.next();
    if let (Some(&syn::TokenTree::Token(syn::Token::Ident(ref ident))),
            Some(&syn::TokenTree::Token(syn::Token::Comma))) = (first, second) {
        if ident == "else" {
            return RHS::Else
        }
    }
    let mut expression = Tokens::new();
    if let Some(&syn::TokenTree::Delimited(syn::Delimited { delim: Brace, .. })) = first {
        first.to_tokens(&mut expression);
        *tts = after_first;
        consume_if_present(tts, syn::Token::Comma);
    } else {
        *tts = start;
        for tt in tts {
            tt.to_tokens(&mut expression);
            if let &syn::TokenTree::Token(syn::Token::Comma) = tt {
                break
            }
        }
    }
    RHS::Expression(expression)
}

fn expand_match_token_macro(to_be_matched: syn::Ident, mut arms: Vec<Arm>) -> Tokens {
    // Handle the last arm specially at the end.
    let last_arm = arms.pop().unwrap();

    // Tags we've seen, used for detecting duplicates.
    let mut seen_tags: HashSet<Tag> = HashSet::new();

    // Case arms for wildcard matching.  We collect these and
    // emit them later.
    let mut wildcards_patterns: Vec<Tokens> = Vec::new();
    let mut wildcards_expressions: Vec<Tokens> = Vec::new();

    // Tags excluded (by an 'else' RHS) from wildcard matching.
    let mut wild_excluded_patterns: Vec<Tokens> = Vec::new();

    let mut arms_code = Vec::new();

    for Arm { binding, lhs, rhs } in arms {
        // Build Rust syntax for the `name @` binding, if any.
        let binding = match binding {
            Some(ident) => quote!(#ident @),
            None => quote!(),
        };

        match (lhs, rhs) {
            (LHS::Pattern(_), RHS::Else) => panic!("'else' may not appear with an ordinary pattern"),

            // ordinary pattern => expression
            (LHS::Pattern(pat), RHS::Expression(expr)) => {
                if !wildcards_patterns.is_empty() {
                    panic!("ordinary patterns may not appear after wildcard tags {:?} {:?}", pat, expr);
                }
                arms_code.push(quote!(#binding #pat => #expr))
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
                    wild_excluded_patterns.push(make_tag_pattern(&Tokens::new(), tag));
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
                            if !wildcards_patterns.is_empty() {
                                panic!("specific tags may not appear after wildcard tags");
                            }

                            if wildcard == Some(true) {
                                panic!("wildcard tags must appear alone");
                            }

                            if wildcard.is_some() {
                                // Push the delimeter `|` if it's not the first tag.
                                arms_code.push(quote!( | ))
                            }
                            arms_code.push(make_tag_pattern(&binding, tag));

                            wildcard = Some(false);
                        }

                        // <_>
                        None => {
                            if wildcard.is_some() {
                                panic!("wildcard tags must appear alone");
                            }
                            wildcard = Some(true);
                            wildcards_patterns.push(make_tag_pattern(&binding, tag));
                            wildcards_expressions.push(expr.clone());
                        }
                    }
                }

                match wildcard {
                    None => panic!("[internal macro error] tag arm with no tags"),
                    Some(false) => arms_code.push(quote!( => #expr)),
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

    quote! {
        match #to_be_matched {
            #(
                #arms_code
            )*
            last_arm_token => {
                let enable_wildcards = match last_arm_token {
                    #(
                        #wild_excluded_patterns => false,
                    )*
                    _ => true,
                };
                match (enable_wildcards, last_arm_token) {
                    #(
                        (true, #wildcards_patterns) => #wildcards_expressions
                    )*
                    (_, #last_pat) => #last_expr
                }
            }
        }
    }
}

fn make_tag_pattern(binding: &Tokens, tag: Tag) -> Tokens {
    let kind = match tag.kind {
        TagKind::StartTag => quote!(::tokenizer::StartTag),
        TagKind::EndTag => quote!(::tokenizer::EndTag),
    };
    let name_field = if let Some(name) = tag.name {
        let name = name.to_string();
        quote!(name: atom!(#name),)
    } else {
        quote!()
    };
    quote! {
        ::tree_builder::types::TagToken(#binding ::tokenizer::Tag { kind: #kind, #name_field .. })
    }
}
