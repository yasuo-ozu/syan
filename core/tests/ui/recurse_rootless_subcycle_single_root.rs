// AUDIT D (SOUNDNESS GAP — fixed → regression): a rootless sub-cycle with <=1 self-referential root
// is now rejected with a clean `abort!` instead of being silently accepted and un-depth-limited.
//
// `#[recurse]`'s soundness guard (`subgraph_is_cyclic`: the SCC minus its roots must be acyclic, else
// the depth never terminates) used to run ONLY on the multi-root path (`build_multiroot_tail`, reached
// when >=2 cycle types self-reference). With <=1 self-referential type, `build_scc` took the
// single-root path and the guard was skipped — so here `A` is the sole root and the `C <-> D`
// sub-cycle, which never touches `A`, threaded the depth param undecremented (un-depth-limited).
//
// The single-root path now runs the same feedback-vertex-set check (`scc \ {root}` must be acyclic),
// so this aborts — matching the long-supported two-self-ref-type rejection
// (`ui/recurse_multiroot_rootless_subcycle.rs`). This test guards that fix.

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
