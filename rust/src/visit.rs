#![allow(dead_code)]
use core::marker::PhantomData;

#[derive(Debug)]
enum Expr<S> {
    Stmt(Box<Stmt<S>>),
    Other(PhantomData<S>),
}

#[derive(Debug)]
enum Stmt<S> {
    Expr(Box<Expr<S>>),
    Other(PhantomData<S>),
}

fn visit_expr<S, V: Visit<S> + ?Sized>(this: &mut V, i: &Expr<S>) {
    match i {
        Expr::Stmt(s) => this.visit_stmt(&*s),
        Expr::Other(_) => (),
    }
}

fn visit_stmt<S, V: Visit<S> + ?Sized>(this: &mut V, i: &Stmt<S>) {
    match i {
        Stmt::Expr(s) => this.visit_expr(&*s),
        Stmt::Other(_) => (),
    }
}

trait IntoVisitor<S, T> {
    fn into_visitor(self) -> impl Visit<S>;
}

trait Visit<S> {
    fn visit_expr(&mut self, i: &Expr<S>) {
        visit_expr(self, i)
    }

    fn visit_stmt(&mut self, i: &Stmt<S>) {
        visit_stmt(self, i)
    }
}

impl<S, T: Visit<S>> Visit<S> for &mut T {
    fn visit_expr(&mut self, i: &Expr<S>) {
        T::visit_expr(self, i)
    }

    fn visit_stmt(&mut self, i: &Stmt<S>) {
        T::visit_stmt(self, i)
    }
}

impl<S, T: Visit<S>> IntoVisitor<S, ()> for T {
    fn into_visitor(self) -> impl Visit<S> {
        self
    }
}

impl<S, F> IntoVisitor<S, Expr<S>> for F
where
    F: FnMut(&Expr<S>),
{
    fn into_visitor(self) -> impl Visit<S> {
        struct Visitor<F>(F);
        impl<F: FnMut(&Expr<S>), S> Visit<S> for Visitor<F> {
            fn visit_expr(&mut self, i: &Expr<S>) {
                self.0(i);
                visit_expr(self, i)
            }
        }
        Visitor(self)
    }
}

impl<S, F> IntoVisitor<S, Stmt<S>> for F
where
    F: FnMut(&Stmt<S>),
{
    fn into_visitor(self) -> impl Visit<S> {
        struct Visitor<F>(F);
        impl<F: FnMut(&Stmt<S>), S> Visit<S> for Visitor<F> {
            fn visit_stmt(&mut self, i: &Stmt<S>) {
                self.0(i);
                visit_stmt(self, i)
            }
        }
        Visitor(self)
    }
}

trait Ast<S> {
    fn visit<T>(&self, visitor: impl IntoVisitor<S, T>) -> &Self;
}

impl<S> Ast<S> for Expr<S> {
    fn visit<T>(&self, visitor: impl IntoVisitor<S, T>) -> &Self {
        let mut visitor = visitor.into_visitor();
        visitor.visit_expr(self);
        self
    }
}

impl<S> Ast<S> for Stmt<S> {
    fn visit<T>(&self, visitor: impl IntoVisitor<S, T>) -> &Self {
        let mut visitor = visitor.into_visitor();
        visitor.visit_stmt(self);
        self
    }
}

fn main() {
    let ast = Expr::Stmt(Box::new(Stmt::Expr(Box::new(Expr::Other(
        PhantomData as PhantomData<()>,
    )))));
    println!("----");
    ast.visit(|expr: &Expr<()>| {
        println!("Expr: {:?}", expr);
    });
    println!("----");
    ast.visit(|stmt: &Stmt<()>| {
        println!("Stmt: {:?}", stmt);
    });
    println!("----");
    ast.visit({
        struct MyVisitor;
        impl<S> Visit<S> for MyVisitor {
            fn visit_expr(&mut self, i: &Expr<S>) {
                println!("expr");
                visit_expr(self, i)
            }

            fn visit_stmt(&mut self, i: &Stmt<S>) {
                println!("stmt");
                visit_stmt(self, i)
            }
        }
        MyVisitor
    });
}
