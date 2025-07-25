use syan::nested::group::GroupParen;
use syan::parse::{Parse, Unparse};

#[derive(Parse, Unparse)]
struct MyStruct {
    a: u32,
    b: u64,
    group: GroupParen<(), ()>,
    #[group(self.group)]
    group_a: String,
}

#[derive(Parse, Unparse)]
enum MyEnum {
    V1 { a: u32, b: u64 },
    V2(f32, f64),
    V3,
}
