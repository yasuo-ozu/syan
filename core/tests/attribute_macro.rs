// use syan::nested::group::GroupParen;
// use syan::parse::{Parse, Unparse};
// use syan::span::{Spanned, WithSpan};
//
// #[allow(unused)]
// #[derive(Clone, Parse, Unparse, Spanned)]
// struct MyStruct<S> {
//     a: WithSpan<u32, S>,
//     b: WithSpan<u64, S>,
//     group: GroupParen<(), S>,
//     #[group(self.group)]
//     group_a: WithSpan<String, S>,
// }
//
// #[allow(unused)]
// #[derive(Parse, Unparse, Spanned)]
// enum MyEnum<S> {
//     V1 {
//         a: WithSpan<u32, S>,
//         b: WithSpan<u64, S>,
//     },
//     V2(WithSpan<f32, S>, WithSpan<f64, S>),
//     V3,
// }
