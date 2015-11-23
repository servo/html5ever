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
in `src/tree_builder/mod.rs`.


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

#![allow(unused_imports)]  // for quotes

use std::collections::{HashSet, HashMap};
use std::collections::hash_map::Entry::{Occupied, Vacant};

use syntax::diagnostic::FatalError;
use syntax::ptr::P;
use syntax::codemap::{Span, Spanned, spanned};
use syntax::ast;
use syntax::parse::parser::{Parser, Restrictions};
use syntax::parse::{token, parser, classify};
use syntax::parse;
use syntax::ext::base::{ExtCtxt, MacResult, MacEager};

use self::TagKind::{StartTag, EndTag};
use self::LHS::{Pat, Tags};
use self::RHS::{Else, Expr};

type Tokens = Vec<ast::TokenTree>;

// FIXME: duplicated in src/tokenizer/interface.rs
#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
enum TagKind {
    StartTag,
    EndTag,
}

impl TagKind {
    /// Turn this `TagKind` into syntax for a literal `tokenizer::TagKind`.
    fn lift(self, cx: &mut ExtCtxt) -> Tokens {
        match self {
            StartTag => quote_tokens!(&mut *cx, ::tokenizer::StartTag),
            EndTag   => quote_tokens!(&mut *cx, ::tokenizer::EndTag),
        }
    }
}

/// A single tag, as may appear in an LHS.
///
/// `name` is `None` for wildcards.
#[derive(PartialEq, Eq, Hash, Clone)]
struct Tag {
    kind: TagKind,
    name: Option<String>,
}

/// Left-hand side of a pattern-match arm.
enum LHS {
    Pat(P<ast::Pat>),
    Tags(Vec<Spanned<Tag>>),
}

/// Right-hand side of a pattern-match arm.
enum RHS {
    Else,
    Expr(P<ast::Expr>),
}

/// A whole arm, including optional outer `name @` binding.
struct Arm {
    binding: Option<ast::SpannedIdent>,
    lhs: Spanned<LHS>,
    rhs: Spanned<RHS>,
}

/// A parsed `match_token!` invocation.
struct Match {
    discriminant: P<ast::Expr>,
    arms: Vec<Arm>,
}

fn push_all<T>(lhs: &mut Vec<T>, rhs: Vec<T>) {
    lhs.extend(rhs.into_iter());
}

fn parse_spanned_ident(parser: &mut Parser) -> Result<ast::SpannedIdent, FatalError> {
    let lo = parser.span.lo;
    let ident = try!(parser.parse_ident());
    let hi = parser.last_span.hi;
    Ok(spanned(lo, hi, ident))
}

fn parse_tag(parser: &mut Parser) -> Result<Spanned<Tag>, FatalError> {
    let lo = parser.span.lo;
    try!(parser.expect(&token::Lt));

    let kind = match try!(parser.eat(&token::BinOp(token::Slash))) {
        true => EndTag,
        false => StartTag,
    };
    let name = match try!(parser.eat(&token::Underscore)) {
        true => None,
        false => Some((*try!(parser.parse_ident()).name.as_str()).to_owned()),
    };

    try!(parser.expect(&token::Gt));
    Ok(spanned(lo, parser.last_span.hi, Tag {
        kind: kind,
        name: name,
    }))
}

/// Parse a `match_token!` invocation into the little AST defined above.
fn parse(cx: &mut ExtCtxt, toks: &[ast::TokenTree]) -> Result<Match, FatalError> {
    let mut parser = parse::new_parser_from_tts(cx.parse_sess(), cx.cfg(), toks.to_vec());

    let discriminant = try!(parser.parse_expr_res(Restrictions::RESTRICTION_NO_STRUCT_LITERAL));
    try!(parser.commit_expr_expecting(&*discriminant, token::OpenDelim(token::Brace)));

    let mut arms: Vec<Arm> = Vec::new();
    while parser.token != token::CloseDelim(token::Brace) {
        let mut binding = None;
        if parser.look_ahead(1, |t| *t == token::At) {
            binding = Some(try!(parse_spanned_ident(&mut parser)));
            try!(parser.bump()); // Consume the @
        }

        let lhs_lo = parser.span.lo;
        let lhs = match parser.token {
            token::Underscore | token::Ident(..) => Pat(try!(parser.parse_pat())),
            token::Lt => {
                let mut tags = Vec::new();
                while parser.token != token::FatArrow {
                    tags.push(try!(parse_tag(&mut parser)));
                }
                Tags(tags)
            }
            _ => return Err(parser.fatal("unrecognized pattern")),
        };
        let lhs_hi = parser.last_span.hi;

        try!(parser.expect(&token::FatArrow));

        let rhs_lo = parser.span.lo;
        let mut rhs_hi = parser.span.hi;
        let rhs = if try!(parser.eat_keyword(token::keywords::Else)) {
            try!(parser.expect(&token::Comma));
            Else
        } else {
            let expr = try!(parser.parse_expr_res(Restrictions::RESTRICTION_STMT_EXPR));
            rhs_hi = parser.last_span.hi;

            let require_comma =
                !classify::expr_is_simple_block(&*expr)
                && parser.token != token::CloseDelim(token::Brace);

            if require_comma {
                try!(parser.commit_expr(
                    &*expr, &[token::Comma], &[token::CloseDelim(token::Brace)]));
            } else {
                try!(parser.eat(&token::Comma));
            }

            Expr(expr)
        };

        arms.push(Arm {
            binding: binding,
            lhs: spanned(lhs_lo, lhs_hi, lhs),
            rhs: spanned(rhs_lo, rhs_hi, rhs),
        });
    }

    // Consume the closing brace
    try!(parser.bump());

    Ok(Match {
        discriminant: discriminant,
        arms: arms,
    })
}

/// Description of a wildcard match arm.
///
/// We defer generating code for these until we process the last, catch-all
/// arm.  This isn't part of the AST produced by `parse()`; it's created
/// while processing that AST.
struct WildcardArm {
    binding: Tokens,
    kind: TagKind,
    expr: P<ast::Expr>,
}

fn make_tag_pattern(cx: &mut ExtCtxt, binding: Tokens, tag: Tag) -> Tokens {
    let kind = tag.kind.lift(cx);
    let mut fields = quote_tokens!(&mut *cx, kind: $kind,);
    match tag.name {
        None => (),
        Some(name) => push_all(&mut fields, quote_tokens!(&mut *cx, name: atom!($name),)),
    }
    quote_tokens!(&mut *cx,
        ::tree_builder::types::TagToken($binding ::tokenizer::Tag { $fields ..})
    )
}

macro_rules! ext_err {
    ($span: expr, $message: expr) => { return Err(($span, $message)) }
}
macro_rules! ext_err_if {
    ($condition: expr, $span: expr, $message: expr) => {
        if $condition { return Err(($span, $message)) }
    }
}

/// Expand the `match_token!` macro.
pub fn expand_to_tokens(cx: &mut ExtCtxt, span: Span, toks: &[ast::TokenTree])
        -> Result<Vec<ast::TokenTree>, (Span, &'static str)> {
    let Match { discriminant, mut arms } = panictry!(parse(cx, toks));

    // Handle the last arm specially at the end.
    let last_arm = match arms.pop() {
        Some(x) => x,
        None => ext_err!(span, "need at least one match arm"),
    };

    // Code for the arms other than the last one.
    let mut arm_code: Tokens = vec!();

    // Tags we've seen, used for detecting duplicates.
    let mut seen_tags: HashSet<Tag> = HashSet::new();

    // Case arms for wildcard matching.  We collect these and
    // emit them later.
    let mut wildcards: Vec<WildcardArm> = vec!();

    // Tags excluded (by an 'else' RHS) from wildcard matching.
    let mut wild_excluded: HashMap<TagKind, Vec<Tag>> = HashMap::new();

    for Arm { binding, lhs, rhs } in arms.into_iter() {
        // Build Rust syntax for the `name @` binding, if any.
        let binding = match binding {
            Some(i) => quote_tokens!(&mut *cx, $i @),
            None => vec!(),
        };

        match (lhs.node, rhs.node) {
            (Pat(_), Else)
                => ext_err!(rhs.span, "'else' may not appear with an ordinary pattern"),

            // ordinary pattern => expression
            (Pat(pat), Expr(expr)) => {
                ext_err_if!(!wildcards.is_empty(), lhs.span,
                    "ordinary patterns may not appear after wildcard tags");
                push_all(&mut arm_code, quote_tokens!(&mut *cx, $binding $pat => $expr,));
            }

            // <tag> <tag> ... => else
            (Tags(tags), Else) => {
                for Spanned { span, node: tag } in tags.into_iter() {
                    ext_err_if!(!seen_tags.insert(tag.clone()), span, "duplicate tag");
                    ext_err_if!(tag.name.is_none(), rhs.span,
                        "'else' may not appear with a wildcard tag");
                    match wild_excluded.entry(tag.kind) {
                        Occupied(e) => { e.into_mut().push(tag.clone()); }
                        Vacant(e)   => { e.insert(vec![tag.clone()]); }
                    }
                }
            }

            // <_> => expression
            // <tag> <tag> ... => expression
            (Tags(tags), Expr(expr)) => {
                // Is this arm a tag wildcard?
                // `None` if we haven't processed the first tag yet.
                let mut wildcard = None;
                for Spanned { span, node: tag } in tags.into_iter() {
                    ext_err_if!(!seen_tags.insert(tag.clone()), span, "duplicate tag");

                    match tag.name {
                        // <tag>
                        Some(_) => {
                            ext_err_if!(!wildcards.is_empty(), lhs.span,
                                "specific tags may not appear after wildcard tags");

                            ext_err_if!(wildcard == Some(true), span,
                                "wildcard tags must appear alone");

                            if wildcard.is_some() {
                                // Push the delimeter `|` if it's not the first tag.
                                push_all(&mut arm_code, quote_tokens!(&mut *cx, |));
                            }
                            push_all(&mut arm_code, make_tag_pattern(cx, binding.clone(), tag));

                            wildcard = Some(false);
                        }

                        // <_>
                        None => {
                            ext_err_if!(wildcard.is_some(), span,
                                "wildcard tags must appear alone");
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
                    None => ext_err!(lhs.span, "[internal macro error] tag arm with no tags"),
                    Some(false) => {
                        push_all(&mut arm_code, quote_tokens!(&mut *cx, => $expr,));
                    }
                    Some(true) => () // codegen for wildcards is deferred
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
    let last_arm_token = token::gensym_ident("last_arm_token");
    let enable_wildcards = token::gensym_ident("enable_wildcards");

    let (last_pat, last_expr) = match (binding, lhs.node, rhs.node) {
        (Some(id), _, _) => ext_err!(id.span, "the last arm cannot have an @-binding"),
        (None, Tags(_), _) => ext_err!(lhs.span, "the last arm cannot have tag patterns"),
        (None, _, Else) => ext_err!(rhs.span, "the last arm cannot use 'else'"),
        (None, Pat(p), Expr(e)) => match p.node {
            ast::PatWild | ast::PatIdent(..) => (p, e),
            _ => ext_err!(lhs.span, "the last arm must have a wildcard or ident pattern"),
        },
    };

    // We can't actually tell if the last pattern is a variable or a nullary enum
    // constructor, but in the latter case rustc will (probably?) give an error
    // about non-exhaustive matching on the expanded `match` expression.

    // Code for the `false` arms inside `let enable_wildcards = ...`.
    let mut enable_wildcards_code = vec!();
    for (_, tags) in wild_excluded.into_iter() {
        for tag in tags.into_iter() {
            push_all(&mut enable_wildcards_code, make_tag_pattern(cx, vec!(), tag));
            push_all(&mut enable_wildcards_code, quote_tokens!(&mut *cx, => false,));
        }
    }

    // Code for the wildcard actions.
    let mut wildcard_code = vec!();
    for WildcardArm { binding, kind, expr } in wildcards.into_iter() {
        let pat = make_tag_pattern(cx, binding, Tag { kind: kind, name: None });
        push_all(&mut wildcard_code, quote_tokens!(&mut *cx,
            (true, $pat) => $expr,
        ));
    }

    // Put it all together!
    Ok(quote_tokens!(&mut *cx,
        match $discriminant {
            $arm_code

            $last_arm_token => {
                let $enable_wildcards = match $last_arm_token {
                    $enable_wildcards_code
                    _ => true,
                };

                match ($enable_wildcards, $last_arm_token) {
                    $wildcard_code
                    (_, $last_pat) => $last_expr,
                }
            },
        }
    ))
}
