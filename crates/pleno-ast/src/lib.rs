pub use pleno_common::Span;
use serde::{Deserialize, Serialize};

pub mod visitor;

#[derive(Debug, Clone, PartialEq)]
pub enum CommentKind {
    Line,
    Block,
    Doc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    pub span: Span,
    pub content: String,
    pub kind: CommentKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Negate,
    Not,
    PreIncrement,
    PostIncrement,
    PreDecrement,
    PostDecrement,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    And,
    Or,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Modulo,
    NullCoalesce,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Literal(Literal),
    Ident(String),
    This,
    Super,
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Property {
        object: Box<Expr>,
        name: String,
    },
    SafeProperty {
        object: Box<Expr>,
        name: String,
    },
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },
    MapLiteral(Vec<MapItem>),
    NamedTupleLiteral(Vec<(String, Expr)>),
    ArrayLiteral(Vec<Expr>),
    TupleLiteral(Vec<Expr>),
    Lambda {
        params: Vec<Param>,
        body: Box<Expr>,
        return_ty: Option<String>,
        annotations: Vec<Annotation>,
    },
    Block(Vec<Stmt>),
    Underscore,
    Index {
        collection: Box<Expr>,
        index: Box<Expr>,
    },
    SafeIndex {
        collection: Box<Expr>,
        index: Box<Expr>,
    },
    Slice {
        collection: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
    },
    SetLiteral(Vec<Expr>),
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
    },
    Pipeline {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    AssertNotNull(Box<Expr>),
    SafeCall {
        callee: Box<Expr>,
        args: Vec<CallArg>,
    },
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Await {
        expr: Box<Expr>,
    },
    Spread(Box<Expr>),
    Paren(Box<Expr>),
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    Is {
        left: Box<Expr>,
        ty: String,
    },
    Specialize {
        left: Box<Expr>,
        type_args: Vec<String>,
    },
    MacroCall {
        name: String,
        args: Vec<CallArg>,
    },
    MacroSplice(Box<Expr>),
    InterpolatedString(Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MapItem {
    Entry(String, Expr),
    Spread(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Literal(Literal),
    Ident(String),
    Tuple(Vec<Pattern>),
    Rest(Option<String>),
    Underscore,
    Variant(String, Vec<Pattern>),
    Object(Vec<(String, Pattern)>, Option<Box<Pattern>>),
    Type(String, Option<String>),
    Or(Vec<Pattern>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
    /// Number of blank lines before this statement (for formatting preservation)
    pub leading_blank_lines: u8,
    /// Trailing inline comment on the same line as this statement
    pub trailing_comment: Option<String>,
}

impl Stmt {
    pub fn new(kind: StmtKind, span: Span) -> Self {
        Self {
            kind,
            span,
            leading_blank_lines: 0,
            trailing_comment: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Module {
        name: String,
        body: Option<Vec<Stmt>>,
        doc: Option<String>,
    },
    Let {
        name: String,
        mutable: bool,
        ty: Option<String>,
        value: Option<Expr>,
        is_const: bool,
        is_deep: bool,
        is_lazy: bool,
        doc: Option<String>,
    },
    MacroDecl {
        name: String,
        params: Vec<String>,
        body: Box<Stmt>,
        doc: Option<String>,
    },
    Assign {
        name: String,
        value: Expr,
    },
    AssignProperty {
        object: Expr,
        name: String,
        value: Expr,
    },
    AssignIndex {
        collection: Expr,
        index: Expr,
        value: Expr,
    },
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    For {
        label: Option<String>,
        names: Vec<String>,
        iterable: Expr,
        body: Box<Stmt>,
    },
    CFor {
        init: Option<Box<Stmt>>,
        condition: Option<Expr>,
        step: Option<Box<Stmt>>,
        body: Box<Stmt>,
    },
    DoWhile {
        body: Box<Stmt>,
        condition: Expr,
    },
    Import {
        path: String,
        kind: ImportKind,
    },
    Export(Box<Stmt>),
    ExportList(Vec<ExportName>),
    Class {
        name: String,
        generics: Vec<GenericParam>,
        fields: Vec<FieldDecl>,
        methods: Vec<FunctionDecl>,
        properties: Vec<PropertyDecl>,
        superclass: Option<String>,
        implements: Vec<String>,
        is_abstract: bool,
        annotations: Vec<Annotation>,
        nested_classes: Vec<Stmt>,
        doc: Option<String>,
    },
    Interface {
        name: String,
        generics: Vec<GenericParam>,
        methods: Vec<FunctionDecl>,
        properties: Vec<PropertyDecl>,
        extends: Vec<String>,
        annotations: Vec<Annotation>,
        doc: Option<String>,
    },
    Enum(EnumDecl),
    Fn {
        name: String,
        generics: Vec<GenericParam>,
        params: Vec<Param>,
        body: Box<Stmt>,
        annotations: Vec<Annotation>,
        is_async: bool,
        doc: Option<String>,
    },
    Try {
        try_branch: Box<Stmt>,
        catch_branches: Vec<CatchBlock>,
        finally_branch: Option<Box<Stmt>>,
    },
    Throw(Expr),
    Return(Option<Expr>),
    Break(Option<String>),
    Continue(Option<String>),
    Labeled {
        label: String,
        stmt: Box<Stmt>,
    },
    Block(Vec<Stmt>),
    TypeAlias {
        name: String,
        ty: String,
        doc: Option<String>,
    },
    Extend {
        target: String,
        generics: Vec<GenericParam>,
        methods: Vec<FunctionDecl>,
        doc: Option<String>,
    },
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchBlock {
    pub name: String,
    pub ty: Option<String>,
    pub body: Box<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
    pub comments: Vec<Comment>,
}

impl Program {
    pub fn new(statements: Vec<Stmt>, comments: Vec<Comment>) -> Self {
        Self {
            statements,
            comments,
        }
    }
}

pub use pleno_common::Variance;

#[derive(Debug, Clone, PartialEq)]
pub struct GenericParam {
    pub name: String,
    pub constraints: Vec<String>, // e.g. T: Comparable
    pub variance: Variance,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub span: Span,
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_ty: Option<String>,
    pub body: Box<Stmt>,
    pub is_static: bool,
    pub is_abstract: bool,
    pub is_async: bool,
    pub visibility: Option<Visibility>,
    pub kind: FunctionKind,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDecl {
    pub span: Span,
    pub name: String,
    pub ty: Option<String>,
    pub getter: Option<FunctionDecl>,
    pub setter: Option<FunctionDecl>,
    pub is_static: bool,
    pub visibility: Option<Visibility>,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub methods: Vec<FunctionDecl>,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub span: Span,
    pub name: String,
    pub params: Option<Vec<Param>>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ownership {
    Move,
    Borrow,
    BorrowMut,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub span: Span,
    pub name: String,
    pub ty: Option<String>,
    pub default: Option<Expr>,
    pub is_variadic: bool,
    pub is_keyword_only: bool,
    pub ownership: Ownership,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub span: Span,
    pub name: String,
    pub ty: Option<String>,
    pub default: Option<Expr>,
    pub is_static: bool,
    pub is_const: bool,
    pub visibility: Option<Visibility>,
    pub annotations: Vec<Annotation>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub name: String,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportName {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportKind {
    Module { alias: Option<String> },
    Names(Vec<ImportName>),
    Star { alias: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportName {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallArg {
    Positional(Expr),
    Named(String, Expr),
    Placeholder,
    Spread(Expr),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionKind {
    Method,
    Getter,
    Setter,
    Operator(String),
}
