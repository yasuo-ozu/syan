#[syan::parse::recurse]
pub mod rec {
    use syan::parse::{Parse, Unparse};

    #[derive(Parse, Unparse)]
    #[predicate($atom: Clone)]
    pub struct A<S> {
        a: Box<B<S>>,
        b: Box<C<S>>,
        _phantom: core::marker::PhantomData<S>,
    }

    #[derive(Parse, Unparse)]
    #[predicate($atom: Clone)]
    pub struct B<S> {
        a: Box<A<S>>,
    }

    #[derive(Parse, Unparse)]
    #[predicate($atom: Clone)]
    pub struct C<S> {
        a: Box<A<S>>,
    }
}

#[syan::parse::recurse]
pub mod complex_cycles {
    use super::rec;
    use std::collections::HashMap;
    use syan::parse::{Parse, Unparse};

    #[derive(Parse, Unparse)]
    #[predicate(K: std::hash::Hash + std::cmp::Eq)]
    #[predicate($atom: Clone)]
    pub struct Node<T, K = String> {
        value: T,
        children: Vec<Box<TreeNode<T, K>>>,
        metadata: NodeMeta<K>,
    }

    #[derive(Parse, Unparse)]
    #[predicate(Graph<T, K>: Parse<$atom>)]
    #[predicate($atom: Clone)]
    pub enum TreeNode<T, K> {
        Leaf {
            data: T,
            parent: Option<Box<Node<T, K>>>,
        },
        Branch {
            nodes: Vec<Node<T, K>>,
            graph: Graph<T, K>,
        },
    }

    #[derive(Parse, Unparse)]
    #[predicate(HashMap<K, Vertex<T, K>>: Parse<$atom>)]
    pub struct Graph<T, K> {
        vertices: HashMap<K, Vertex<T, K>>,
        edges: Vec<Edge<T, K>>,
        external_ref: Option<Box<rec::A<T>>>,
    }

    #[derive(Parse, Unparse)]
    #[predicate(K: std::hash::Hash + std::cmp::Eq)]
    #[predicate($atom: Clone)]
    pub enum Vertex<T, K> {
        Simple(T),
        Complex {
            node: Box<Node<T, K>>,
            neighbors: Vec<Box<Vertex<T, K>>>,
        },
    }

    #[derive(Parse, Unparse)]
    #[predicate(K: std::hash::Hash + std::cmp::Eq)]
    #[predicate($atom: Clone)]
    pub struct Edge<T, K> {
        from: Box<Vertex<T, K>>,
        to: Box<Vertex<T, K>>,
        weight: Option<T>,
        graph_ref: Box<Graph<T, K>>,
    }

    #[derive(Parse, Unparse)]
    #[predicate(K: std::hash::Hash + std::cmp::Eq)]
    #[predicate($atom: Clone)]
    pub enum NodeMeta<K> {
        Empty,
        Tagged(K),
        Nested {
            inner: Box<NodeMeta<K>>,
            tree_ref: Option<Box<TreeNode<(), K>>>,
        },
    }
}

#[syan::parse::recurse]
pub mod cross_module_refs {
    use super::{complex_cycles, rec};
    use syan::parse::{Parse, Unparse};

    #[derive(Parse, Unparse)]
    #[predicate($atom: Clone)]
    pub enum MultiRef<T> {
        Simple(T),
        RecRef(Box<rec::A<T>>),
        ComplexRef(Box<complex_cycles::Node<T>>),
        SelfRef(Box<MultiRef<T>>),
        Combination {
            simple: Box<SimpleChain<T>>,
            external: complex_cycles::Graph<T, String>,
        },
    }

    #[derive(Parse, Unparse)]
    #[predicate($atom: Clone)]
    pub struct SimpleChain<T> {
        value: T,
        next: Option<Box<SimpleChain<T>>>,
        multi_ref: Box<MultiRef<T>>,
    }
}
