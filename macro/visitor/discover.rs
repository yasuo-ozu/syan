use super::*;

/// Resolvable paths of `def`'s field types that are *followed* (head in `subast`) but neither
/// visited/inherited (a method call) nor self (already in `done`) — i.e. unlisted intermediates to
/// fetch so they can be drilled through.
pub(crate) fn followed_intermediates(
    def: &Item,
    subast: &[SubEntry],
    method_set: &HashSet<String>,
    self_ident: Option<&str>,
) -> Vec<Path> {
    let mut user_types: HashSet<String> = subast.iter().map(|e| e.key.to_string()).collect();
    if let Some(s) = self_ident {
        user_types.insert(s.to_string());
    }
    let mut out = Vec::new();
    for_each_field_type(def, &mut |ty| {
        discover_followed(ty, subast, method_set, self_ident, &user_types, &mut out)
    });
    out
}

/// Recurse a field type (descending into tuple elements) collecting followed-but-unlisted
/// intermediate paths to fetch for inline drilling.
fn discover_followed(
    ty: &Type,
    subast: &[SubEntry],
    method_set: &HashSet<String>,
    self_ident: Option<&str>,
    user_types: &HashSet<String>,
    out: &mut Vec<Path>,
) {
    if let Some(p) = peel(ty, user_types) {
        match &p.head {
            // Tuple element types must be inspected too (a followed type may be nested in a tuple, or
            // a tuple nested behind containers — `Vec<(Cast, Type)>`).
            Head::Tuple(elems) => {
                for elem in elems {
                    discover_followed(elem, subast, method_set, self_ident, user_types, out);
                }
            }
            Head::Path { head, .. } => {
                let hs = head.to_string();
                if Some(hs.as_str()) == self_ident {
                    return; // self -> already in `done`
                }
                if let Some(e) = subast.iter().find(|e| &e.key == head) {
                    // Fetch only when the entry's *real* type isn't visited/inherited (else a method,
                    // already fetched under its `visitor!(..)` path — even when the head is aliased).
                    if !method_set.contains(&last_ident(&e.path).to_string()) {
                        out.push(e.path.clone());
                    }
                }
            }
        }
    }
}

fn for_each_field_type(def: &Item, f: &mut dyn FnMut(&Type)) {
    match def {
        Item::Enum(e) => {
            for v in &e.variants {
                for field in &v.fields {
                    f(&field.ty);
                }
            }
        }
        Item::Struct(s) => {
            for field in &s.fields {
                f(&field.ty);
            }
        }
        _ => {}
    }
}

// Module generation.

/// Mint a generated helper param ident whose name avoids every name in `reserved` (the visited
/// types' generic params), appending `_` until free. Rust rejects two generic params with the same
/// name string in one item regardless of hygiene, so this — not just `mixed_site` — is what lets a
/// visited type declare a param literally named `__V`/etc. The `mixed_site` span is kept for extra
/// isolation from other call-site idents.
pub(crate) fn fresh_ident(base: &str, reserved: &HashSet<String>) -> Ident {
    let mut name = base.to_string();
    while reserved.contains(&name) {
        name.push('_');
    }
    Ident::new(&name, Span::mixed_site())
}

/// Like [`fresh_ident`] but for an indexed family `<prefix>0..<prefix>{max}` (tuple closure params):
/// returns a prefix such that no `<prefix>{i}` collides with `reserved`.
pub(crate) fn fresh_prefix(base: &str, reserved: &HashSet<String>, max: usize) -> String {
    let mut prefix = base.to_string();
    while (0..max).any(|i| reserved.contains(&format!("{prefix}{i}"))) {
        prefix.push('_');
    }
    prefix
}
