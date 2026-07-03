//! Visitor codegen audit regression pins: union lifetime order, followed-ref mut leaf,
//! recurse helper hygiene, and non-root lifetime threading. Compiling is the check.
#![allow(dead_code)]
// `visitor!()` fetching a `#[recurse]` module's metadata misattributes a spurious "unused import"
// to the `use ... recurse;` line, though `#[recurse]` does use it. Cosmetic span artifact.
#![allow(unused_imports)]

// Union of visited-type params must be normalized lifetime-first even when a type param is
// listed before a lifetime param.
mod union_lifetime_order {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub struct Outer<S> {
        pub _p: PhantomData<S>,
    }

    #[derive(Ast)]
    #[subast()]
    pub struct Inner<'a> {
        pub _p: PhantomData<&'a ()>,
    }

    mod v {
        // Outer (type param `S`) listed BEFORE Inner (lifetime `'a`) → union order is `[S, 'a]`.
        syan::visit::visitor!(crate::union_lifetime_order::Outer, crate::union_lifetime_order::Inner);
    }

    #[test]
    fn union_orders_lifetime_first() {
        let o = Outer::<()> { _p: PhantomData };
        o.visit(|_x: &Inner<'_>| {});
    }
}

// A followed shared-reference field (`&T`) must be a leaf on the mut side, else `&mut **r`
// through a `&` is E0596. The shared side still visits it.
mod followed_ref_mut {
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub struct Leaf<S> {
        pub _p: core::marker::PhantomData<S>,
    }

    #[derive(Ast)]
    #[subast(crate::followed_ref_mut::Leaf)]
    pub struct Holder<'a, S> {
        pub r: &'a Leaf<S>,
    }

    mod v {
        syan::visit::visitor!(crate::followed_ref_mut::Holder, crate::followed_ref_mut::Leaf);
    }

    #[test]
    fn followed_shared_ref_field_is_visitable() {}
}

// The recurse/mixed visitor path must fresh-name its helper params (`__V`/`__W`/`__R{i}`)
// against the visited types' param names; the cycle type here declares a `__V` param.
mod recurse_helper_hygiene {
    use syan::parse::recurse;

    #[recurse]
    mod m {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        pub enum Expr<S, __V> {
            Nest(Box<Expr<S, __V>>),
            Lit(PhantomData<(S, __V)>),
        }
    }

    mod v {
        syan::visit::visitor!(crate::recurse_helper_hygiene::m::Expr);
    }

    #[test]
    fn helper_idents_do_not_collide_with_a_cycle_param() {}
}

// A non-root cycle type's extra lifetime param must be emitted lifetime-first in the
// generated `visit_*` method generic list.
mod recurse_nonroot_lifetime {
    use syan::parse::recurse;

    #[recurse]
    mod m {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        pub enum Expr<S> {
            Stmt(Box<Stmt<'static, S>>),
            Lit(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast()]
        pub enum Stmt<'a, S> {
            Back(Box<Expr<S>>),
            Tag(PhantomData<(&'a (), S)>),
        }
    }

    mod v {
        syan::visit::visitor!(crate::recurse_nonroot_lifetime::m::Expr, crate::recurse_nonroot_lifetime::m::Stmt);
    }

    #[test]
    fn nonroot_extra_lifetime_threads_through() {}
}
