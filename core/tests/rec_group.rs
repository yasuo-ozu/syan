use syan::parse::Parse;

#[derive(Parse)]
pub struct GroupParenExample<S> {
    pub paren_token: syan::nested::group::GroupParen<(), S>,
    #[group(self.paren_token)]
    pub inner_value: (),
}
