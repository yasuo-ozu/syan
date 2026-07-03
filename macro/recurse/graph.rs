use super::*;

pub(crate) fn collect_refs(ty: &Type, known: &HashSet<String>, out: &mut HashSet<String>) {
    match ty {
        Type::Path(TypePath { path, .. }) => {
            if let Some(seg) = path.segments.first() {
                let name = seg.ident.to_string();
                if known.contains(&name) {
                    out.insert(name);
                }
            }
            for seg in &path.segments {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    for arg in &ab.args {
                        if let GenericArgument::Type(t) = arg {
                            collect_refs(t, known, out);
                        }
                    }
                }
            }
        }
        Type::Reference(r) => collect_refs(&r.elem, known, out),
        Type::Slice(s) => collect_refs(&s.elem, known, out),
        Type::Array(a) => collect_refs(&a.elem, known, out),
        Type::Tuple(t) => t.elems.iter().for_each(|e| collect_refs(e, known, out)),
        _ => {}
    }
}

pub(crate) fn collect_refs_fields(fields: &Fields, known: &HashSet<String>, out: &mut HashSet<String>) {
    for field in fields {
        collect_refs(&field.ty, known, out);
    }
}

pub(crate) fn collect_refs_item(item: &Item, known: &HashSet<String>) -> HashSet<String> {
    let mut out = HashSet::new();
    match item {
        Item::Enum(e) => e
            .variants
            .iter()
            .for_each(|v| collect_refs_fields(&v.fields, known, &mut out)),
        Item::Struct(s) => collect_refs_fields(&s.fields, known, &mut out),
        _ => {}
    }
    out
}

// Collect only direct (outermost type constructor) references — used to pick the root type.
pub(crate) fn collect_direct_refs_item(item: &Item, known: &HashSet<String>) -> HashSet<String> {
    let mut out = HashSet::new();
    let check = |ty: &Type, out: &mut HashSet<String>| {
        if let Type::Path(TypePath { path, .. }) = ty {
            if let Some(seg) = path.segments.first() {
                let name = seg.ident.to_string();
                if known.contains(&name) {
                    out.insert(name);
                }
            }
        }
    };
    match item {
        Item::Enum(e) => {
            for v in &e.variants {
                for field in &v.fields {
                    check(&field.ty, &mut out);
                }
            }
        }
        Item::Struct(s) => {
            for field in &s.fields {
                check(&field.ty, &mut out);
            }
        }
        _ => {}
    }
    out
}

/// The *recursive* strongly-connected components of the reference `graph`, via Tarjan's algorithm
/// (`safegraph`). Each returned set is one **independent cycle**: a non-trivial SCC (mutual recursion
/// of size > 1, including longer cycles) or a singleton SCC carrying a self-loop (a directly self-
/// referential type). Non-recursive singletons are omitted. Two types share a set iff they are
/// mutually reachable, so independent cycles in one module come back as *separate* sets — each gets
/// its own recurse machinery. The Vec is sorted by each SCC's least type name for deterministic codegen.
pub(crate) fn find_cycle_sccs(graph: &HashMap<String, HashSet<String>>) -> Vec<HashSet<String>> {
    use safegraph::algo::connectivity::tarjan_scc;
    use safegraph::graph::Graph;
    use safegraph::BTreeGraph;

    // Build the directed reference graph. `safegraph`'s map-backed graph keys nodes by their value
    // (which must be `Copy`), so each type name gets a small `u32` id (its position in `names`);
    // edges carry a bare unique counter (edges are keyed by value too).
    let names: Vec<&String> = graph.keys().collect();
    let id_of: HashMap<&str, u32> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i as u32))
        .collect();

    let mut g = BTreeGraph::<u32, u32>::default();
    let node_ix: Vec<_> = (0..names.len() as u32)
        .map(|i| g.insert_node(i).unwrap())
        .collect();
    let mut edge_id = 0u32;
    for (from, tos) in graph {
        let fi = node_ix[id_of[from.as_str()] as usize];
        for to in tos {
            if let Some(&tid) = id_of.get(to.as_str()) {
                g.push_edge(edge_id, [fi, node_ix[tid as usize]]).unwrap();
                edge_id += 1;
            }
        }
    }

    let mut sccs: Vec<HashSet<String>> = Vec::new();
    for scc in tarjan_scc(&g) {
        if scc.len() > 1 {
            sccs.push(scc.iter().map(|&n| names[*g.node(n) as usize].clone()).collect());
        } else {
            let name = names[*g.node(scc[0]) as usize];
            if graph.get(name).is_some_and(|refs| refs.contains(name)) {
                sccs.push(std::iter::once(name.clone()).collect());
            }
        }
    }
    sccs.sort_by(|a, b| a.iter().min().cmp(&b.iter().min()));
    sccs
}

/// Is the subgraph induced by the cycle's **non-root** types cyclic? Used as the multi-root soundness
/// guard: the depth only decrements at a self-referential root, so a cycle running entirely through
/// non-root types would never terminate. Built and tested with `safegraph` (same `u32`-keyed graph as
/// `find_cycle_sccs`, restricted to `scc \ root_types`).
pub(crate) fn subgraph_is_cyclic(
    scc: &HashSet<String>,
    root_types: &HashSet<String>,
    type_refs: &HashMap<String, HashSet<String>>,
) -> bool {
    use safegraph::algo::connectivity::is_cyclic_directed;
    use safegraph::graph::Graph;
    use safegraph::BTreeGraph;

    let nodes: Vec<&String> = scc.iter().filter(|n| !root_types.contains(*n)).collect();
    let id_of: HashMap<&str, u32> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i as u32))
        .collect();
    let mut g = BTreeGraph::<u32, u32>::default();
    let node_ix: Vec<_> = (0..nodes.len() as u32)
        .map(|i| g.insert_node(i).unwrap())
        .collect();
    let mut edge_id = 0u32;
    for n in &nodes {
        let fi = node_ix[id_of[n.as_str()] as usize];
        for to in type_refs.get(n.as_str()).into_iter().flatten() {
            if let Some(&tid) = id_of.get(to.as_str()) {
                g.push_edge(edge_id, [fi, node_ix[tid as usize]]).unwrap();
                edge_id += 1;
            }
        }
    }
    is_cyclic_directed(&g)
}
