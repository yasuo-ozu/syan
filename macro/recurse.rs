use proc_macro2::TokenStream;
use proc_macro_error::abort;
use std::collections::{HashMap, HashSet};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::*;
use template_quote::quote;

fn get_name_and_field_tys(item: &Item) -> Option<(Ident, Vec<Type>)> {
    match item {
        Item::Struct(ItemStruct { ident, fields, .. }) => Some((
            ident.clone(),
            fields.iter().map(|field| field.ty.clone()).collect(),
        )),
        Item::Enum(ItemEnum {
            ident, variants, ..
        }) => Some((
            ident.clone(),
            variants
                .iter()
                .map(|variant| variant.fields.iter().map(|field| field.ty.clone()))
                .flatten()
                .collect(),
        )),
        _ => None,
    }
}

fn find_fundamental_tys(
    ty: &Type,
    idents: &[Ident],
    additional_predicates: &mut Vec<TokenStream>,
    additional_predicates_parse: &mut Vec<TokenStream>,
    additional_predicates_unparse: &mut Vec<TokenStream>,
) -> Option<HashSet<Type>> {
    let child_types = match ty {
        Type::Array(TypeArray { elem, .. }) => vec![elem.as_ref()],
        Type::Ptr(TypePtr { elem, .. }) => vec![elem.as_ref()],
        Type::Reference(TypeReference { elem, .. }) => vec![elem.as_ref()],
        Type::Slice(TypeSlice { elem, .. }) => vec![elem.as_ref()],
        Type::Tuple(TypeTuple { elems, .. }) => elems.iter().collect(),
        Type::Path(TypePath { path, .. }) => {
            // Collect types from generic arguments
            let mut types = Vec::new();
            for segment in &path.segments {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let GenericArgument::Type(arg_ty) = arg {
                            types.push(arg_ty);
                        }
                    }
                }
            }
            types
        }
        _ => vec![],
    };

    let mut result = HashSet::new();
    let mut has_fundamental = false;

    let fundamental_tys = child_types
        .iter()
        .map(|ty| {
            find_fundamental_tys(
                ty,
                idents,
                additional_predicates,
                additional_predicates_parse,
                additional_predicates_unparse,
            )
        })
        .collect::<Vec<_>>();

    for (fund, &ty) in fundamental_tys.iter().zip(&child_types) {
        if let Some(inner) = fund {
            has_fundamental = true;
            result.extend(inner.clone());
        } else {
            result.insert(ty.clone());
        }
    }

    if let Type::Path(TypePath { qself: None, path }) = ty {
        if path.leading_colon.is_none() && path.segments.len() == 1 {
            let segment = &path.segments[0];
            let type_name = &segment.ident;

            if idents.contains(type_name) {
                // This is a fundamental type
                return Some([ty.clone()].into());
            }
        }
        if let Some(_) = compare_trait_path(
            [
                &parse_quote!(::core::option::Option<T>),
                &parse_quote!(::std::option::Option<T>),
                &parse_quote!(::core::vec::Vec<T>),
                &parse_quote!(::std::vec::Vec<T>),
            ],
            path,
        ) {
            additional_predicates_parse.push(quote!($atom: ::core::clone::Clone));
        } else if let Some(args) =
            compare_trait_path([&parse_quote!(::std::collections::HashSet<K>)], path)
        {
            additional_predicates.push(quote!(#{&args[0]}: ::core::hash::Hash + ::core::cmp::Eq));
        } else if let Some(args) =
            compare_trait_path([&parse_quote!(::std::collections::BTreeSet<K>)], path)
        {
            additional_predicates.push(quote!(#{&args[0]}: ::core::cmp::Ord));
        } else if let Some(args) =
            compare_trait_path([&parse_quote!(::std::collections::HashMap<K, V>)], path)
        {
            assert_eq!(fundamental_tys.len(), 2);
            match (&fundamental_tys[0], &fundamental_tys[1]) {
                (None, None) => {
                    additional_predicates_parse.push(quote!(
                         <#{&args[0]} as $syan::parse::parse::Parse<$atom>>:: Error: $syan::error::UnionWith<
                             <#{&args[1]} as $syan::parse::parse::Parse<$atom>>::Error
                         >
                    ));
                }
                (Some(_), None) => {
                    additional_predicates_parse.push(quote!(
                         $syan::error::ParseError: $syan::error::UnionWith<
                             <#{&args[1]} as $syan::parse::parse::Parse<$atom>>::Error
                         >
                    ));
                }
                (None, Some(_)) => {
                    additional_predicates_parse.push(quote!(
                         <#{&args[0]} as $syan::parse::parse::Parse<$atom>>:: Error: $syan::error::UnionWith<
                             $syan::error::ParseError
                         >
                    ));
                }
                _ => (),
            }
            additional_predicates.push(quote!(#{&args[0]}: ::core::hash::Hash + ::core::cmp::Eq));
        } else if let Some(args) =
            compare_trait_path([&parse_quote!(::std::collections::BTreeMap<K, V>)], path)
        {
            assert_eq!(fundamental_tys.len(), 2);
            match (&fundamental_tys[0], &fundamental_tys[1]) {
                (None, None) => {
                    additional_predicates_parse.push(quote!(
                         <#{&args[0]} as $syan::parse::parse::Parse<$atom>>:: Error: $syan::error::UnionWith<
                             <#{&args[1]} as $syan::parse::parse::Parse<$atom>>::Error
                         >
                    ));
                }
                (Some(_), None) => {
                    additional_predicates_parse.push(quote!(
                         $syan::error::ParseError: $syan::error::UnionWith<
                             <#{&args[1]} as $syan::parse::parse::Parse<$atom>>::Error
                         >
                    ));
                }
                (None, Some(_)) => {
                    additional_predicates_parse.push(quote!(
                         <#{&args[0]} as $syan::parse::parse::Parse<$atom>>:: Error: $syan::error::UnionWith<
                             $syan::error::ParseError
                         >
                    ));
                }
                _ => (),
            }
            additional_predicates.push(quote!(#{&args[0]}: ::core::cmp::Ord));
        }
    }

    if has_fundamental {
        Some(result)
    } else {
        None
    }
}

fn make_graph(items: &[Item]) -> HashMap<Ident, HashSet<Ident>> {
    let mut ret: HashMap<Ident, HashSet<Ident>> = HashMap::new();
    let mut idents = HashSet::new();

    for item in items {
        if let Some((ident, _)) = get_name_and_field_tys(item) {
            idents.insert(ident);
        }
    }

    use syn::visit::Visit;
    #[derive(Default)]
    struct Visitor(HashSet<Ident>);
    impl syn::visit::Visit<'_> for Visitor {
        fn visit_type(&mut self, ty: &Type) {
            if let Type::Path(TypePath { qself: None, path }) = ty {
                if path.leading_colon.is_none() && path.segments.len() == 1 {
                    self.0.insert(path.segments[0].ident.clone());
                }
                syn::visit::visit_path(self, path);
            }
        }
    }

    // Build the graph using get_name_and_field_tys
    for item in items {
        if let Some((ident, field_tys)) = get_name_and_field_tys(item) {
            let mut visitor = Visitor::default();
            for field_ty in field_tys {
                visitor.visit_type(&field_ty);
            }
            ret.entry(ident)
                .or_default()
                .extend(visitor.0.intersection(&idents).cloned().collect::<Vec<_>>());
        }
    }
    ret
}

fn find_strong_loop(graph: &HashMap<Ident, HashSet<Ident>>) -> Vec<HashSet<Ident>> {
    let mut tarjan_indices: HashMap<Ident, (usize, usize)> = HashMap::new();

    fn visit(
        counter: &mut usize,
        node: &Ident,
        graph: &HashMap<Ident, HashSet<Ident>>,
        tarjan_indices: &mut HashMap<Ident, (usize, usize)>,
        stack: &mut Vec<Ident>,
        output: &mut Vec<HashSet<Ident>>,
    ) {
        // check if the node is not visited
        if tarjan_indices.contains_key(node) {
            return;
        }
        tarjan_indices.insert(node.clone(), (*counter, *counter));
        let stack_top = stack.len();
        stack.push(node.clone());
        *counter += 1;
        for next_node in &graph[node] {
            match tarjan_indices.get(next_node) {
                Some((next_ix, _)) if stack.contains(next_node) => {
                    let next_ix = *next_ix;
                    let (_, lowlink) = tarjan_indices.get_mut(node).unwrap();
                    *lowlink = usize::min(*lowlink, next_ix);
                }
                None => {
                    visit(counter, next_node, graph, tarjan_indices, stack, output);
                    let next_lowlink = tarjan_indices.get(next_node).unwrap().1;
                    let (_, lowlink) = tarjan_indices.get_mut(node).unwrap();
                    *lowlink = usize::min(*lowlink, next_lowlink);
                }
                _ => (),
            }
        }
        let (ix, lowlink) = tarjan_indices.get(node).unwrap();
        if lowlink == ix {
            let mut to_output = HashSet::new();
            while stack.len() > stack_top {
                to_output.insert(stack.pop().unwrap());
            }
            output.push(to_output);
        }
    }

    let mut counter = 0;
    let mut stack = Vec::new();
    let mut output = Vec::new();
    for node in graph.keys() {
        visit(
            &mut counter,
            node,
            graph,
            &mut tarjan_indices,
            &mut stack,
            &mut output,
        );
    }
    output
}

fn compare_trait_path<'a>(
    absolute_paths: impl IntoIterator<Item = &'a Path>,
    mandatory_path: &Path,
) -> Option<Punctuated<GenericArgument, Token![,]>> {
    let mand_segments = &mandatory_path.segments;

    for absolute_path in absolute_paths {
        let abs_segments = &absolute_path.segments;
        let mut matched_count = 0;
        for (abs_seg, mand_seg) in abs_segments.iter().rev().zip(mand_segments.iter().rev()) {
            if abs_seg.ident == mand_seg.ident {
                matched_count += 1;
            } else {
                break;
            }
        }

        if matched_count == mand_segments.len()
            && matched_count > 0
            && (matched_count < abs_segments.len()
                || (absolute_path.leading_colon.is_some()
                    || mandatory_path.leading_colon.is_none()))
        {
            match (
                &abs_segments.last().unwrap().arguments,
                &mand_segments.last().unwrap().arguments,
            ) {
                (
                    PathArguments::AngleBracketed(_),
                    PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }),
                ) => {
                    return Some(args.clone());
                }
                (PathArguments::None, PathArguments::None) => return Some(Default::default()),
                _ => (),
            }
        }
    }
    None
}

fn split_type_path(ty: &Type) -> Option<(&Ident, Punctuated<GenericArgument, Token![,]>)> {
    if let Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: None,
            segments,
        },
        ..
    }) = ty
    {
        if segments.len() == 1 {
            let segment = &segments[0];
            match &segment.arguments {
                PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) => {
                    return Some((&segment.ident, args.clone()));
                }
                PathArguments::None => {
                    // For types without generic arguments, we need to handle them differently
                    return Some((&segment.ident, Punctuated::new()));
                }
                _ => {}
            }
        }
    }
    None
}

#[allow(unused)]
fn collect_all_fundamental_tys(
    ty: &Type,
    items: &HashMap<Ident, (Punctuated<GenericParam, Token![,]>, Vec<Type>)>,
    hist_stack: &mut Vec<(Ident, Punctuated<GenericArgument, Token![,]>)>,
    additional_predicates: &mut Vec<TokenStream>,
    additional_predicates_parse: &mut Vec<TokenStream>,
    additional_predicates_unparse: &mut Vec<TokenStream>,
) -> Option<HashSet<Type>> {
    let idents: Vec<Ident> = items.keys().cloned().collect();
    let mut result: HashSet<Type> = HashSet::new();
    let mut has_fundamental = false;
    for fundamental_ty in find_fundamental_tys(
        ty,
        &idents,
        additional_predicates,
        additional_predicates_parse,
        additional_predicates_unparse,
    )
    .unwrap_or(core::iter::once(ty.clone()).collect())
    {
        if let Some((ident, args)) = split_type_path(&fundamental_ty) {
            if let Some((params, tys)) = items.get(ident) {
                has_fundamental = true;
                if let Some((_, found_args)) =
                    hist_stack.iter().find(|(hist_name, _)| hist_name == ident)
                {
                    continue;
                }
                hist_stack.push((ident.clone(), args.clone()));
                let mut param_map = HashMap::new();
                for (param, arg) in params.iter().zip(args.iter()) {
                    param_map.insert(param.clone(), arg.clone());
                }
                // Handle default type parameters if args.len() < params.len()
                if args.len() < params.len() {
                    for param in params.iter().skip(args.len()) {
                        if let GenericParam::Type(TypeParam {
                            default: Some(default_ty),
                            ..
                        }) = param
                        {
                            param_map
                                .insert(param.clone(), GenericArgument::Type(default_ty.clone()));
                        }
                    }
                }
                for ty in tys {
                    let mut substituted_ty = ty.clone();
                    substitute_generics_in_type(&mut substituted_ty, &param_map);
                    if let Some(nested_result) = collect_all_fundamental_tys(
                        &substituted_ty,
                        items,
                        hist_stack,
                        additional_predicates,
                        additional_predicates_parse,
                        additional_predicates_unparse,
                    ) {
                        has_fundamental = true;
                        result.extend(nested_result);
                    } else {
                        result.insert(substituted_ty.clone());
                    }
                }
                hist_stack.pop();
                continue;
            }
        }
        result.insert(fundamental_ty.clone());
    }
    if has_fundamental {
        Some(result)
    } else {
        None
    }
}

// Helper function to substitute generics in a type using VisitMut
fn substitute_generics_in_type(ty: &mut Type, param_map: &HashMap<GenericParam, GenericArgument>) {
    use syn::visit_mut::VisitMut;

    struct Visitor<'a>(&'a HashMap<GenericParam, GenericArgument>);

    impl<'a> VisitMut for Visitor<'a> {
        fn visit_type_mut(&mut self, ty: &mut Type) {
            // Replace types using the map
            if let Type::Path(TypePath { qself: None, path }) = ty {
                if path.leading_colon.is_none() && path.segments.len() == 1 {
                    let segment = &path.segments[0];
                    // Look for matching GenericParam::Type with this ident
                    for (param, replacement) in self.0.iter() {
                        if let GenericParam::Type(TypeParam { ident, .. }) = param {
                            if ident == &segment.ident {
                                if let GenericArgument::Type(replacement_ty) = replacement {
                                    *ty = replacement_ty.clone();
                                    return; // Don't continue visiting if we replaced
                                }
                            }
                        }
                    }
                }
            }
            syn::visit_mut::visit_type_mut(self, ty);
        }

        fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
            // Replace lifetimes using the map
            for (param, replacement) in self.0.iter() {
                if let GenericParam::Lifetime(LifetimeParam {
                    lifetime: param_lifetime,
                    ..
                }) = param
                {
                    if param_lifetime.ident == lifetime.ident {
                        if let GenericArgument::Lifetime(replacement_lifetime) = replacement {
                            *lifetime = replacement_lifetime.clone();
                            return;
                        }
                    }
                }
            }
        }

        fn visit_expr_mut(&mut self, expr: &mut Expr) {
            // Replace const generics in expressions
            if let Expr::Path(expr_path) = expr {
                if let Some(path) = &expr_path.path.get_ident() {
                    for (param, replacement) in self.0.iter() {
                        if let GenericParam::Const(ConstParam { ident, .. }) = param {
                            if ident == *path {
                                if let GenericArgument::Const(replacement_expr) = replacement {
                                    *expr = replacement_expr.clone();
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            syn::visit_mut::visit_expr_mut(self, expr);
        }
    }

    let mut visitor = Visitor(param_map);
    visitor.visit_type_mut(ty);
}

pub fn recurse(mut item_mod: ItemMod) -> TokenStream {
    let span = item_mod.span();
    let contents = item_mod
        .content
        .as_mut()
        .unwrap_or_else(|| abort!(span, "no content"))
        .1
        .as_mut_slice();
    let items: HashMap<_, _> = contents
        .iter()
        .filter_map(|item| match item {
            Item::Struct(ItemStruct {
                ident,
                generics,
                fields,
                ..
            }) => {
                let field_tys = fields.iter().map(|f| f.ty.clone()).collect();
                Some((ident.clone(), (generics.params.clone(), field_tys)))
            }
            Item::Enum(ItemEnum {
                ident,
                generics,
                variants,
                ..
            }) => {
                let field_tys = variants
                    .iter()
                    .flat_map(|v| v.fields.iter().map(|f| f.ty.clone()))
                    .collect();
                Some((ident.clone(), (generics.params.clone(), field_tys)))
            }
            _ => None,
        })
        .collect();
    let graph = make_graph(&contents);
    let groups = find_strong_loop(&graph);
    for content in contents {
        let (ident, generics, all_fields) = match content {
            Item::Struct(ItemStruct {
                generics,
                ident,
                fields,
                ..
            }) => (
                ident.clone(),
                generics.clone(),
                fields.iter_mut().collect::<Vec<_>>(),
            ),
            Item::Enum(ItemEnum {
                generics,
                ident,
                variants,
                ..
            }) => (
                ident.clone(),
                generics.clone(),
                variants
                    .iter_mut()
                    .flat_map(|v| &mut v.fields)
                    .collect::<Vec<_>>(),
            ),
            _ => continue,
        };
        let group = groups
            .iter()
            .find_map(|g| g.contains(&ident).then_some(g))
            .unwrap();
        let items = items
            .iter()
            .filter(|(ident, _)| group.contains(ident))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut fundamental_tys: Option<HashSet<Type>> = None;
        let mut additional_predicates = Vec::new();
        let mut additional_predicates_parse = Vec::new();
        let mut additional_predicates_unparse = Vec::new();
        let mut stack = Vec::new();
        stack.push((
            ident.clone(),
            generics
                .params
                .iter()
                .map(|p| match p {
                    GenericParam::Lifetime(LifetimeParam { lifetime, .. }) => {
                        GenericArgument::Lifetime(lifetime.clone())
                    }
                    GenericParam::Type(TypeParam { ident, .. }) => parse_quote!(#ident),
                    GenericParam::Const(ConstParam { ident, .. }) => parse_quote!(#ident),
                })
                .collect(),
        ));
        for field in all_fields {
            if let Some(tys) = collect_all_fundamental_tys(
                &field.ty,
                &items,
                &mut stack,
                &mut additional_predicates,
                &mut additional_predicates_parse,
                &mut additional_predicates_unparse,
            ) {
                let mut s = fundamental_tys.unwrap_or_default();
                s.extend(tys);
                fundamental_tys = Some(s);
                field.attrs.push(parse_quote!(#[ignore_bounds]));
            }
        }
        if let Some(fundamental_tys) = &fundamental_tys {
            match content {
                Item::Struct(ItemStruct { attrs, .. }) | Item::Enum(ItemEnum { attrs, .. }) => {
                    let s = fundamental_tys.iter().collect::<Vec<_>>();
                    attrs.push(parse_quote!(#[fundamental_tys(#(#s),*)]));
                }
                _ => panic!(),
            }
        }
        if !additional_predicates.is_empty() {
            match content {
                Item::Struct(ItemStruct { attrs, .. }) | Item::Enum(ItemEnum { attrs, .. }) => {
                    for predicate in &additional_predicates {
                        attrs.push(parse_quote!(#[predicate(#predicate)]));
                    }
                }
                _ => panic!(),
            }
        }
        if !additional_predicates_parse.is_empty() {
            match content {
                Item::Struct(ItemStruct { attrs, .. }) | Item::Enum(ItemEnum { attrs, .. }) => {
                    for predicate in &additional_predicates_parse {
                        attrs.push(parse_quote!(#[predicate_parse(#predicate)]));
                    }
                }
                _ => panic!(),
            }
        }
        if !additional_predicates_unparse.is_empty() {
            match content {
                Item::Struct(ItemStruct { attrs, .. }) | Item::Enum(ItemEnum { attrs, .. }) => {
                    for predicate in &additional_predicates_unparse {
                        attrs.push(parse_quote!(#[predicate_unparse(#predicate)]));
                    }
                }
                _ => panic!(),
            }
        }
    }
    quote!(#item_mod)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::format_ident;

    macro_rules! graph {
        ($($node:ident $(=> $($edge:ident),+)? $(;)*)*) => {{
            #[allow(unused_mut)]
            let mut graph: HashMap<Ident, HashSet<Ident>> = HashMap::new();
            $(
                let node = format_ident!(stringify!($node));
                #[allow(unused_mut)]
                let mut edges = HashSet::new();
                $($(
                    let edge = format_ident!(stringify!($edge));
                    edges.insert(edge.clone());
                    graph.entry(edge).or_default();
                )+)?
                graph.insert(node, edges);
            )*
            graph
        }};
    }

    #[test]
    fn test_find_strong_loop_empty_graph() {
        let graph = graph!();
        let result = find_strong_loop(&graph);
        let expected: Vec<HashSet<Ident>> = vec![];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_strong_loop_single_node_no_self_loop() {
        let graph = graph!(A);
        let a = format_ident!("A");

        let result = find_strong_loop(&graph);
        let expected = vec![HashSet::from([a])];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_strong_loop_single_node_self_loop() {
        let graph = graph!(A => A);
        let a = format_ident!("A");

        let result = find_strong_loop(&graph);
        let expected = vec![HashSet::from([a])];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_strong_loop_two_nodes_no_cycle() {
        let graph = graph!(A => B; B);
        let a = format_ident!("A");
        let b = format_ident!("B");

        let mut result = find_strong_loop(&graph);
        result.sort_by_key(|set| set.iter().next().unwrap().to_string());
        let mut expected = vec![HashSet::from([a]), HashSet::from([b])];
        expected.sort_by_key(|set| set.iter().next().unwrap().to_string());
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_strong_loop_two_nodes_cycle() {
        let graph = graph!(A => B; B => A);
        let a = format_ident!("A");
        let b = format_ident!("B");

        let result = find_strong_loop(&graph);
        let expected = vec![HashSet::from([a, b])];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_strong_loop_three_nodes_cycle() {
        let graph = graph!(A => B; B => C; C => A);
        let a = format_ident!("A");
        let b = format_ident!("B");
        let c = format_ident!("C");

        let result = find_strong_loop(&graph);
        let expected = vec![HashSet::from([a, b, c])];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_strong_loop_complex_graph() {
        // A -> B -> C -> A (cycle of 3)
        // B -> D
        // E (isolated)
        let graph = graph!(A => B; B => C, D; C => A; D; E);
        let a = format_ident!("A");
        let b = format_ident!("B");
        let c = format_ident!("C");
        let d = format_ident!("D");
        let e = format_ident!("E");

        let result = find_strong_loop(&graph);

        // Check that we have exactly 3 SCCs
        assert_eq!(result.len(), 3);

        // Verify specific SCCs exist
        let expected_sccs = vec![
            HashSet::from([d]),       // isolated D
            HashSet::from([e]),       // isolated E
            HashSet::from([a, b, c]), // 3-node cycle
        ];

        for expected_scc in expected_sccs {
            assert!(
                result.contains(&expected_scc),
                "SCC {:?} not found in result",
                expected_scc
            );
        }
    }

    #[test]
    fn test_find_strong_loop_multiple_cycles() {
        // A <-> B (cycle of 2)
        // C <-> D (cycle of 2)
        let graph = graph!(A => B; B => A; C => D; D => C);
        let a = format_ident!("A");
        let b = format_ident!("B");
        let c = format_ident!("C");
        let d = format_ident!("D");

        let mut result = find_strong_loop(&graph);
        result.sort_by_key(|set| set.iter().next().unwrap().to_string());
        let mut expected = vec![
            HashSet::from([a, b]), // A <-> B cycle
            HashSet::from([c, d]), // C <-> D cycle
        ];
        expected.sort_by_key(|set| set.iter().next().unwrap().to_string());
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_strong_loop_large_complex_graph() {
        // Large complex graph with multiple SCCs of different sizes
        //
        // SCC 1: A -> B -> C -> A (3-node cycle)
        // SCC 2: D -> E -> F -> G -> D (4-node cycle)
        // SCC 3: H <-> I (2-node cycle)
        // SCC 4: J -> K -> L -> M -> N -> J (5-node cycle)
        // Isolated nodes: O, P, Q
        // Cross-SCC edges: A -> D, C -> H, G -> O, I -> P
        let graph = graph!(
            A => B, D;
            B => C;
            C => A, H;
            D => E;
            E => F;
            F => G;
            G => D, O;
            H => I;
            I => H, P;
            J => K;
            K => L;
            L => M;
            M => N;
            N => J;
            O;
            P;
            Q
        );

        let a = format_ident!("A");
        let b = format_ident!("B");
        let c = format_ident!("C");
        let d = format_ident!("D");
        let e = format_ident!("E");
        let f = format_ident!("F");
        let g = format_ident!("G");
        let h = format_ident!("H");
        let i = format_ident!("I");
        let j = format_ident!("J");
        let k = format_ident!("K");
        let l = format_ident!("L");
        let m = format_ident!("M");
        let n = format_ident!("N");
        let o = format_ident!("O");
        let p = format_ident!("P");
        let q = format_ident!("Q");

        let result = find_strong_loop(&graph);

        // Check that we have exactly 7 SCCs with the right sizes
        assert_eq!(result.len(), 7);

        let mut size_counts = std::collections::HashMap::new();
        for scc in &result {
            *size_counts.entry(scc.len()).or_insert(0) += 1;
        }

        assert_eq!(size_counts[&1], 3); // 3 isolated nodes
        assert_eq!(size_counts[&2], 1); // 1 two-node cycle
        assert_eq!(size_counts[&3], 1); // 1 three-node cycle
        assert_eq!(size_counts[&4], 1); // 1 four-node cycle
        assert_eq!(size_counts[&5], 1); // 1 five-node cycle

        // Verify specific SCCs exist
        let expected_sccs = vec![
            HashSet::from([o]),             // Isolated node O
            HashSet::from([p]),             // Isolated node P
            HashSet::from([q]),             // Isolated node Q
            HashSet::from([h, i]),          // H <-> I (2-node cycle)
            HashSet::from([a, b, c]),       // A -> B -> C -> A (3-node cycle)
            HashSet::from([d, e, f, g]),    // D -> E -> F -> G -> D (4-node cycle)
            HashSet::from([j, k, l, m, n]), // J -> K -> L -> M -> N -> J (5-node cycle)
        ];

        for expected_scc in expected_sccs {
            assert!(
                result.contains(&expected_scc),
                "SCC {:?} not found in result",
                expected_scc
            );
        }
    }

    #[test]
    fn test_compare_trait_path_comprehensive() {
        macro_rules! test_success {
            ($abs:ty, $mand:ty $(,)?) => {
                let abs_path: Path = parse_quote!($abs);
                let mand_path: Path = parse_quote!($mand);
                let result = compare_trait_path([&abs_path], &mand_path);
                assert!(result.is_some());
            };
        }

        macro_rules! test_failure {
            ($abs:ty, $mand:ty $(,)?) => {
                let abs_path: Path = parse_quote!($abs);
                let mand_path: Path = parse_quote!($mand);
                let result = compare_trait_path([&abs_path], &mand_path);
                assert!(result.is_none());
            };
        }

        // Success cases
        test_success!(std::collections::HashMap, std::collections::HashMap);
        test_success!(
            std::collections::HashMap<K, V>,
            std::collections::HashMap<A, B>,
        );
        test_success!(
            crate::std::collections::HashMap<String, i32>,
            std::collections::HashMap<C, D>,
        );
        test_success!(HashMap<T>, HashMap<T>);
        test_success!(::std::collections::HashMap<T>, HashMap<T>);
        test_success!(::std::collections::HashMap, HashMap);
        test_success!(::std::collections::HashMap, std::collections::HashMap);
        test_success!(
            std::collections::HashMap<String, Vec<i32>>,
            HashMap<A, B>,
        );
        test_success!(
            crate::module::std::collections::HashMap<T>,
            std::collections::HashMap<T>,
        );

        // Failure cases
        test_failure!(std::collections::HashSet, HashMap);
        test_failure!(HashMap, std::collections::HashMap);
        test_failure!(crate::other::HashMap, std::collections::HashMap);
    }

    #[test]
    fn test_graph_macro_edge_cases() {
        // Test various edge cases of the graph macro syntax

        // Single node with multiple outgoing edges
        let graph1 = graph!(A => B, C, D, E, F);
        assert_eq!(graph1.len(), 6); // A + B + C + D + E + F
        assert_eq!(graph1[&format_ident!("A")].len(), 5);

        // Multiple nodes, some with edges, some without
        let graph2 = graph!(X => Y, Z; A; B => C; D);
        assert_eq!(graph2.len(), 7); // X + Y + Z + A + B + C + D
        assert_eq!(graph2[&format_ident!("X")].len(), 2);
        assert_eq!(graph2[&format_ident!("A")].len(), 0);
        assert_eq!(graph2[&format_ident!("B")].len(), 1);
        assert_eq!(graph2[&format_ident!("D")].len(), 0);

        // Chain of nodes
        let graph3 = graph!(A => B; B => C; C => D; D => E; E => F);
        assert_eq!(graph3.len(), 6);
        for node_name in ["A", "B", "C", "D", "E"] {
            let node = format_ident!("{}", node_name);
            assert_eq!(
                graph3[&node].len(),
                1,
                "Node {} should have exactly 1 outgoing edge",
                node_name
            );
        }
        assert_eq!(graph3[&format_ident!("F")].len(), 0);
    }

    #[test]
    fn test_collect_all_fundamental_tys() {
        // Define test structs and enums using quote! macro
        let test_module: ItemMod = parse_quote! {
            mod test {
                struct CircularA<T> {
                    b_ref: CircularB<T>,
                    data: u32,
                }

                struct CircularB<T> {
                    c_ref: CircularC<T>,
                    value: String,
                }

                struct CircularC<T> {
                    a_ref: CircularA<T>,
                    b_ref: CircularB<T>,
                    flag: bool,
                }
            }
        };
        let expected_types: HashSet<String> = [
            quote!(u32).to_string(),
            quote!(String).to_string(),
            quote!(bool).to_string(),
            // quote!(CircularB<i32>).to_string(),
        ]
        .into();

        let items: HashMap<Ident, (Punctuated<GenericParam, Token![,]>, Vec<Type>)> = test_module
            .content
            .unwrap()
            .1
            .iter()
            .filter_map(|item| match item {
                Item::Struct(ItemStruct {
                    ident,
                    generics,
                    fields,
                    ..
                }) => {
                    let field_tys = fields.iter().map(|f| f.ty.clone()).collect();
                    Some((ident.clone(), (generics.params.clone(), field_tys)))
                }
                Item::Enum(ItemEnum {
                    ident,
                    generics,
                    variants,
                    ..
                }) => {
                    let field_tys = variants
                        .iter()
                        .flat_map(|v| v.fields.iter().map(|f| f.ty.clone()))
                        .collect();
                    Some((ident.clone(), (generics.params.clone(), field_tys)))
                }
                _ => None,
            })
            .collect();

        let mut hist_stack = Vec::new();
        let mut additional_predicates = Vec::new();

        let result = collect_all_fundamental_tys(
            &parse_quote!(CircularA<i32>),
            &items,
            &mut hist_stack,
            &mut additional_predicates,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .unwrap()
        .into_iter()
        .map(|t| quote!(#t).to_string())
        .collect::<HashSet<_>>();
        assert_eq!(&result, &expected_types);
        assert_eq!(hist_stack.len(), 0);

        let result = collect_all_fundamental_tys(
            &parse_quote!(CircularB<i32>),
            &items,
            &mut hist_stack,
            &mut additional_predicates,
            &mut Vec::new(),
            &mut Vec::new(),
        );
        let circular_b_expected: HashSet<Type> =
            [parse_quote!(u32), parse_quote!(String), parse_quote!(bool)].into();
        assert_eq!(result.as_ref(), Some(&circular_b_expected));
        assert_eq!(hist_stack.len(), 0);

        let result = collect_all_fundamental_tys(
            &parse_quote!(CircularC<i32>),
            &items,
            &mut hist_stack,
            &mut additional_predicates,
            &mut Vec::new(),
            &mut Vec::new(),
        );
        let circular_c_expected: HashSet<Type> =
            [parse_quote!(u32), parse_quote!(String), parse_quote!(bool)].into();
        assert_eq!(result.as_ref(), Some(&circular_c_expected));
        assert_eq!(hist_stack.len(), 0);

        let result = collect_all_fundamental_tys(
            &parse_quote!(Option<CircularA<i32>>),
            &items,
            &mut hist_stack,
            &mut additional_predicates,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .unwrap()
        .into_iter()
        .map(|t| quote!(#t).to_string())
        .collect::<HashSet<_>>();
        assert_eq!(&result, &expected_types);
        assert_eq!(hist_stack.len(), 0);
    }

    #[test]
    fn test_recurse_mutual_reference() {
        let input: ItemMod = parse_quote! {
            mod test {
                struct A {
                    id: u32,
                    b_field: B,
                    name: String,
                }
                struct B {
                    value: i32,
                    a_field: A,
                    flag: bool,
                }
            }
        };

        let result = recurse(input);
        println!("result = {}", quote!(#result));
        let output: ItemMod = parse2(result).unwrap();

        let items = &output.content.unwrap().1;
        // Check struct A
        if let Item::Struct(struct_a) = &items[0] {
            assert_eq!(struct_a.ident, "A");

            // Parse #[fundamental_tys] attribute
            let fundamental_attr = struct_a
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("fundamental_tys"))
                .expect("A should have #[fundamental_tys]");

            let types: Punctuated<Type, Token![,]> = fundamental_attr
                .parse_args_with(Punctuated::parse_terminated)
                .expect("Failed to parse fundamental_tys args");
            let actual_types: HashSet<String> =
                types.into_iter().map(|t| quote!(#t).to_string()).collect();
            assert!(
                actual_types.len() == 2
                    && actual_types.contains("bool")
                    && actual_types.contains("i32")
            );

            // Check fields: only b_field should have #[ignore_bounds]
            let fields: Vec<_> = struct_a.fields.iter().collect();
            assert_eq!(fields.len(), 3);

            let id_has_ignore = fields[0]
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("ignore_bounds"));
            assert!(!id_has_ignore, "id field should NOT have #[ignore_bounds]");

            let b_has_ignore = fields[1]
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("ignore_bounds"));
            assert!(b_has_ignore, "b_field should have #[ignore_bounds]");

            let name_has_ignore = fields[2]
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("ignore_bounds"));
            assert!(
                !name_has_ignore,
                "name field should NOT have #[ignore_bounds]"
            );
        }

        // Check struct B
        if let Item::Struct(struct_b) = &items[1] {
            assert_eq!(struct_b.ident, "B");

            // Parse #[fundamental_tys] attribute
            let fundamental_attr = struct_b
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("fundamental_tys"))
                .expect("B should have #[fundamental_tys]");

            let types: Punctuated<Type, Token![,]> = fundamental_attr
                .parse_args_with(Punctuated::parse_terminated)
                .expect("Failed to parse fundamental_tys args");
            let actual_types: HashSet<String> =
                types.into_iter().map(|t| quote!(#t).to_string()).collect();
            assert!(
                actual_types.len() == 2
                    && actual_types.contains("String")
                    && actual_types.contains("u32")
            );

            // Check fields: only a_field should have #[ignore_bounds]
            let fields: Vec<_> = struct_b.fields.iter().collect();
            assert_eq!(fields.len(), 3);

            let value_has_ignore = fields[0]
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("ignore_bounds"));
            assert!(
                !value_has_ignore,
                "value field should NOT have #[ignore_bounds]"
            );

            let a_has_ignore = fields[1]
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("ignore_bounds"));
            assert!(a_has_ignore, "a_field should have #[ignore_bounds]");

            let flag_has_ignore = fields[2]
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("ignore_bounds"));
            assert!(
                !flag_has_ignore,
                "flag field should NOT have #[ignore_bounds]"
            );
        }
    }
}
