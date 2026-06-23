// AUDIT (missing diagnostic): a union listed in visitor!() is silently dropped. item_ident /
// item_generics return None for Item::Union, so a (fetched) union never becomes a visit target —
// no visit_* method, no inherent method, and no diagnostic. When mixed with a real type it is
// dropped silently; when it is the ONLY listed type you get the misleading "no AST definitions
// resolved for the visitor" abort (pointing at #[derive(Ast)]) rather than "unions cannot be
// visited". Fix: detect Item::Union among targets and abort with a clear message.
#[derive(Clone, Copy, syan::visit::Ast)]
pub union U {
    pub a: u32,
    pub b: f32,
}

pub mod visit {
    syan::visit::visitor!(crate::U);
}

fn main() {}
