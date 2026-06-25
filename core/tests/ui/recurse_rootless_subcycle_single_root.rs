// AUDIT (SOUNDNESS GAP — RED until fixed): a rootless sub-cycle with <=1 self-referential root is
// silently accepted and is NOT depth-limited.
//
// `#[recurse]`'s soundness guard (`subgraph_is_cyclic`: the SCC minus its self-referential roots must
// be acyclic, else the depth never terminates) is consulted ONLY on the multi-root path
// (`build_multiroot_tail`, reached when >=2 cycle types self-reference). With <=1 self-referential
// type, `build_scc` takes the single-root path and the guard is never run — so here `A` is the sole
// (heuristic) root and the `C <-> D` sub-cycle, which never touches `A`, threads the depth param
// `__Rec` undecremented and is silently un-depth-limited.
//
// The two-self-ref-type version of this exact shape IS cleanly rejected
// (`ui/recurse_multiroot_rootless_subcycle.rs`). This is a `compile_fail` test that is RED today
// (the module wrongly COMPILES). EXPECTED (when fixed): `#[recurse]` runs the feedback-vertex-set
// check on the single-root path too and aborts with the same clear message — then bless the `.stderr`.

use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum A<S> {
        Me(Box<A<S>>), // the ONLY self-reference -> sole root
        ToC(Box<C<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum C<S> {
        ToD(Box<D<S>>),
        ToA(Box<A<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum D<S> {
        ToC(Box<C<S>>), // C <-> D sub-cycle, never touching the root A
        Lit(PhantomData<S>),
    }
}

fn main() {}
