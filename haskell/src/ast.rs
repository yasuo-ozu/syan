use syan::span::{WithSpan};

#[derive(Clone, Debug, PartialEq)]
pub struct Module<S> {
    pub name: WithSpan<ModuleName<S>, S>,
    pub exports: Option<WithSpan<ExportList<S>, S>>,
    pub imports: Vec<WithSpan<Import<S>, S>>,
    pub decls: Vec<WithSpan<Declaration<S>, S>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModuleName<S> {
    pub name: WithSpan<String, S>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportList<S> {
    pub exports: Vec<WithSpan<Export<S>, S>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Export<S> {
    Var(WithSpan<String, S>),
    TyCon(WithSpan<String, S>),
    Module(WithSpan<ModuleName<S>, S>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Import<S> {
    pub qualified: bool,
    pub module: WithSpan<ModuleName<S>, S>,
    pub as_name: Option<WithSpan<ModuleName<S>, S>>,
    pub imports: Option<WithSpan<ImportList<S>, S>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportList<S> {
    pub hiding: bool,
    pub items: Vec<WithSpan<ImportSpec<S>, S>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImportSpec<S> {
    Var(WithSpan<String, S>),
    TyCon(WithSpan<String, S>, Vec<WithSpan<String, S>>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Declaration<S> {
    TypeSig(WithSpan<TypeSignature<S>, S>),
    FunBind(WithSpan<FunctionBinding<S>, S>),
    DataDecl(WithSpan<DataDeclaration<S>, S>),
    TypeDecl(WithSpan<TypeDeclaration<S>, S>),
    ClassDecl(WithSpan<ClassDeclaration<S>, S>),
    InstDecl(WithSpan<InstanceDeclaration<S>, S>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeSignature<S> {
    pub names: Vec<WithSpan<String, S>>,
    pub type_: WithSpan<Type<S>, S>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionBinding<S> {
    pub name: WithSpan<String, S>,
    pub patterns: Vec<WithSpan<Pattern<S>, S>>,
    pub rhs: WithSpan<RightHandSide<S>, S>,
    pub where_clause: Option<WithSpan<Vec<WithSpan<Declaration<S>, S>>, S>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RightHandSide<S> {
    Simple(WithSpan<Expression<S>, S>),
    Guarded(Vec<WithSpan<GuardedRhs<S>, S>>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuardedRhs<S> {
    pub guards: Vec<WithSpan<Expression<S>, S>>,
    pub expr: WithSpan<Expression<S>, S>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataDeclaration<S> {
    pub name: WithSpan<String, S>,
    pub params: Vec<WithSpan<String, S>>,
    pub constructors: Vec<WithSpan<Constructor<S>, S>>,
    pub deriving: Vec<WithSpan<String, S>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Constructor<S> {
    pub name: WithSpan<String, S>,
    pub fields: Vec<WithSpan<Type<S>, S>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeDeclaration<S> {
    pub name: WithSpan<String, S>,
    pub params: Vec<WithSpan<String, S>>,
    pub type_: WithSpan<Type<S>, S>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClassDeclaration<S> {
    pub context: Vec<WithSpan<Type<S>, S>>,
    pub name: WithSpan<String, S>,
    pub param: WithSpan<String, S>,
    pub methods: Vec<WithSpan<Declaration<S>, S>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InstanceDeclaration<S> {
    pub context: Vec<WithSpan<Type<S>, S>>,
    pub class: WithSpan<String, S>,
    pub instance: WithSpan<Type<S>, S>,
    pub methods: Vec<WithSpan<Declaration<S>, S>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Type<S> {
    Var(WithSpan<String, S>),
    Con(WithSpan<String, S>),
    App(Box<WithSpan<Type<S>, S>>, Box<WithSpan<Type<S>, S>>),
    Arrow(Box<WithSpan<Type<S>, S>>, Box<WithSpan<Type<S>, S>>),
    Tuple(Vec<WithSpan<Type<S>, S>>),
    List(Box<WithSpan<Type<S>, S>>),
    Paren(Box<WithSpan<Type<S>, S>>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expression<S> {
    Var(WithSpan<String, S>),
    Con(WithSpan<String, S>),
    Lit(WithSpan<Literal, S>),
    App(Box<WithSpan<Expression<S>, S>>, Box<WithSpan<Expression<S>, S>>),
    InfixApp(Box<WithSpan<Expression<S>, S>>, WithSpan<String, S>, Box<WithSpan<Expression<S>, S>>),
    Lambda(Vec<WithSpan<Pattern<S>, S>>, Box<WithSpan<Expression<S>, S>>),
    Let(Vec<WithSpan<Declaration<S>, S>>, Box<WithSpan<Expression<S>, S>>),
    If(Box<WithSpan<Expression<S>, S>>, Box<WithSpan<Expression<S>, S>>, Box<WithSpan<Expression<S>, S>>),
    Case(Box<WithSpan<Expression<S>, S>>, Vec<WithSpan<Alternative<S>, S>>),
    Tuple(Vec<WithSpan<Expression<S>, S>>),
    List(Vec<WithSpan<Expression<S>, S>>),
    Paren(Box<WithSpan<Expression<S>, S>>),
    Section(WithSpan<SectionKind<S>, S>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SectionKind<S> {
    LeftSection(Box<WithSpan<Expression<S>, S>>, WithSpan<String, S>),
    RightSection(WithSpan<String, S>, Box<WithSpan<Expression<S>, S>>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Alternative<S> {
    pub pattern: WithSpan<Pattern<S>, S>,
    pub rhs: WithSpan<RightHandSide<S>, S>,
    pub where_clause: Option<WithSpan<Vec<WithSpan<Declaration<S>, S>>, S>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pattern<S> {
    Var(WithSpan<String, S>),
    Con(WithSpan<String, S>),
    Lit(WithSpan<Literal, S>),
    Wildcard,
    As(WithSpan<String, S>, Box<WithSpan<Pattern<S>, S>>),
    App(WithSpan<String, S>, Vec<WithSpan<Pattern<S>, S>>),
    Tuple(Vec<WithSpan<Pattern<S>, S>>),
    List(Vec<WithSpan<Pattern<S>, S>>),
    Paren(Box<WithSpan<Pattern<S>, S>>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    Char(char),
    String(String),
}