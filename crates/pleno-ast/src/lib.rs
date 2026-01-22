#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Negate,
    Not,
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
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
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },
    MapLiteral(Vec<(String, Expr)>),
    ArrayLiteral(Vec<Expr>),
    TupleLiteral(Vec<Expr>),
    Underscore,
    Index {
        collection: Box<Expr>,
        index: Box<Expr>,
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
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Literal(Literal),
    Ident(String),
    Tuple(Vec<Pattern>),
    Underscore,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Module {
        name: String,
    },
    Let {
        name: String,
        mutable: bool,
        ty: Option<String>,
        value: Option<Expr>,
        is_const: bool,
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
        name: String,
        iterable: Expr,
        body: Box<Stmt>,
    },
    Import {
        path: String,
        kind: ImportKind,
    },
    Export(Box<Stmt>),
    ExportList(Vec<ExportName>),
    Class {
        name: String,
        fields: Vec<FieldDecl>,
        methods: Vec<FunctionDecl>,
        superclass: Option<String>,
        annotations: Vec<Annotation>,
    },
    Fn {
        name: String,
        params: Vec<Param>,
        body: Box<Stmt>,
        annotations: Vec<Annotation>,
    },
    Return(Option<Expr>),
    Break,
    Continue,
    Block(Vec<Stmt>),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

impl Program {
    pub fn new(statements: Vec<Stmt>) -> Self {
        Self { statements }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_ty: Option<String>,
    pub body: Box<Stmt>,
    pub is_static: bool,
    pub visibility: Option<Visibility>,
    pub kind: FunctionKind,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Option<String>,
    pub default: Option<Expr>,
    pub is_variadic: bool,
    pub is_keyword_only: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub name: String,
    pub is_static: bool,
    pub visibility: Option<Visibility>,
    pub annotations: Vec<Annotation>,
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
}
