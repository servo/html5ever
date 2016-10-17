// Copyright 2016 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// Just enough of an AST visitor to reach every expression.

use syn;

pub trait Visitor {
    fn visit_crate(&mut self, _crate: &mut syn::Crate) {}
    fn visit_item(&mut self, _item: &mut syn::Item) {}
    fn visit_trait_item(&mut self, _item: &mut syn::TraitItem) {}
    fn visit_impl_item(&mut self, _item: &mut syn::ImplItem) {}
    fn visit_block(&mut self, _expr: &mut syn::Block) {}
    fn visit_statement(&mut self, _expr: &mut syn::Stmt) {}
    fn visit_expression(&mut self, _expr: &mut syn::Expr) {}
}

pub struct RecursiveVisitor<V> {
    pub node_visitor: V
}

impl<V: Visitor> Visitor for RecursiveVisitor<V> {
    fn visit_crate(&mut self, crate_: &mut syn::Crate) {
        self.node_visitor.visit_crate(crate_);
        for item in &mut crate_.items {
            self.visit_item(item)
        }
    }

    fn visit_item(&mut self, item: &mut syn::Item) {
        use syn::ItemKind::*;
        self.node_visitor.visit_item(item);
        match item.node {
            ExternCrate(_) => {}
            Use(_) => {}
            Static(_, _, ref mut expr) => self.visit_expression(expr),
            Const(_, ref mut expr) => self.visit_expression(expr),
            Fn(_, _, _, _, _, ref mut block) => self.visit_block(block),
            Mod(ref mut items) => {
                for item in items {
                    self.visit_item(item)
                }
            }
            ForeignMod(_) => {}
            Ty(_, _) => {}
            Enum(_, _) => {}
            Struct(_, _) => {}
            Union(_, _) => {}
            Trait(_, _, _, ref mut trait_items) => {
                for trait_item in trait_items {
                    self.visit_trait_item(trait_item)
                }
            }
            DefaultImpl(_, _) => {}
            Impl(_, _, _, _, _, ref mut impl_items) => {
                for impl_item in impl_items {
                    self.visit_impl_item(impl_item)
                }
            }
            Mac(_) => {}
        }
    }

    fn visit_trait_item(&mut self, trait_item: &mut syn::TraitItem) {
        use syn::TraitItemKind::*;
        self.node_visitor.visit_trait_item(trait_item);
        match trait_item.node {
            Const(_, Some(ref mut expr)) => self.visit_expression(expr),
            Const(_, None) => {}
            Method(_, Some(ref mut block)) => self.visit_block(block),
            Method(_, None) => {}
            Type(_, _) => {}
            Macro(_) => {}
        }
    }

    fn visit_impl_item(&mut self, impl_item: &mut syn::ImplItem) {
        use syn::ImplItemKind::*;
        self.node_visitor.visit_impl_item(impl_item);
        match impl_item.node {
            Const(_, ref mut expr) => self.visit_expression(expr),
            Method(_, ref mut block) => self.visit_block(block),
            Type(_) => {}
            Macro(_) => {}
        }
    }

    fn visit_block(&mut self, block: &mut syn::Block) {
        self.node_visitor.visit_block(block);
        for statement in &mut block.stmts {
            self.visit_statement(statement)
        }
    }

    fn visit_statement(&mut self, statement: &mut syn::Stmt) {
        use syn::Stmt::*;
        self.node_visitor.visit_statement(statement);
        match *statement {
            Local(ref mut local) => {
                if let Some(ref mut expr) = local.init {
                    self.visit_expression(expr)
                }
            }
            Item(ref mut item) => self.visit_item(item),
            Expr(ref mut expr) => self.visit_expression(expr),
            Semi(ref mut expr) => self.visit_expression(expr),
            Mac(_) => {}
        }
    }

    fn visit_expression(&mut self, expr: &mut syn::Expr) {
        use syn::Expr::*;
        self.node_visitor.visit_expression(expr);
        match *expr {
            Box(ref mut boxed) => {
                self.visit_expression(boxed)
            }
            Vec(ref mut elements) => {
                for element in elements {
                    self.visit_expression(element)
                }
            }
            Call(ref mut called, ref mut args) => {
                self.visit_expression(called);
                for arg in args {
                    self.visit_expression(arg)
                }
            }
            MethodCall(_, _, ref mut args) => {
                for arg in args {
                    self.visit_expression(arg)
                }
            }
            Tup(ref mut elements) => {
                for element in elements {
                    self.visit_expression(element)
                }
            }
            Binary(_, ref mut left, ref mut right) => {
                self.visit_expression(left);
                self.visit_expression(right);
            }
            Unary(_, ref mut operand) => {
                self.visit_expression(operand)
            }
            Lit(_) => {}
            Cast(ref mut expr, _) => {
                self.visit_expression(expr)
            }
            Type(ref mut expr, _) => {
                self.visit_expression(expr)
            }
            If(ref mut test, ref mut then, ref mut else_) => {
                self.visit_expression(test);
                self.visit_block(then);
                if let Some(ref mut else_) = *else_ {
                    self.visit_expression(else_);
                }
            }
            IfLet(_, ref mut test, ref mut then, ref mut else_) => {
                self.visit_expression(test);
                self.visit_block(then);
                if let Some(ref mut else_) = *else_ {
                    self.visit_expression(else_);
                }
            }
            While(ref mut test, ref mut block, _) => {
                self.visit_expression(test);
                self.visit_block(block);
            }
            WhileLet(_, ref mut test, ref mut block, _) => {
                self.visit_expression(test);
                self.visit_block(block);
            }
            ForLoop(_, ref mut iterable, ref mut block, _) => {
                self.visit_expression(iterable);
                self.visit_block(block);
            }
            Loop(ref mut block, _) => {
                self.visit_block(block);
            }
            Match(ref mut matched, ref mut arms) => {
                self.visit_expression(matched);
                for arm in arms {
                    if let Some(ref mut guard) = arm.guard {
                        self.visit_expression(guard)
                    }
                    self.visit_expression(&mut arm.body)
                }
            }
            Closure(_, _, ref mut block) => {
                self.visit_block(block)
            }
            Block(_, ref mut block) => {
                self.visit_block(block)
            }
            Assign(ref mut left, ref mut right) => {
                self.visit_expression(left);
                self.visit_expression(right);
            }
            AssignOp(_, ref mut left, ref mut right) => {
                self.visit_expression(left);
                self.visit_expression(right);
            }
            Field(ref mut base, _) => {
                self.visit_expression(base)
            }
            TupField(ref mut base, _) => {
                self.visit_expression(base)
            }
            Index(ref mut base, ref mut index) => {
                self.visit_expression(base);
                self.visit_expression(index);
            }
            Range(ref mut start, ref mut end, _) => {
                if let Some(ref mut start) = *start {
                    self.visit_expression(start)
                }
                if let Some(ref mut end) = *end {
                    self.visit_expression(end)
                }
            }
            Path(_, _) => {}
            AddrOf(_, ref mut base) => {
                self.visit_expression(base)
            }
            Break(_) => {}
            Continue(_) => {}
            Ret(Some(ref mut expr)) => {
                self.visit_expression(expr)
            }
            Ret(None) => {}
            Mac(_) => {}
            Struct(_, ref mut fields, ref mut base) => {
                for field in fields {
                    self.visit_expression(&mut field.expr)
                }
                if let Some(ref mut base) = *base {
                    self.visit_expression(base)
                }
            }
            Repeat(ref mut element, ref mut number) => {
                self.visit_expression(element);
                self.visit_expression(number);
            }
            Paren(ref mut expr) => {
                self.visit_expression(expr)
            }
            Try(ref mut expr) => {
                self.visit_expression(expr)
            }
        }
    }
}
