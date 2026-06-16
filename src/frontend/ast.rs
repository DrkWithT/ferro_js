
use crate::frontend::token::Token;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operator {
    Noop,
    NegBool,
    ForceNum,
    NegNum,
    New,
    Delete,
    TypeOf,
    Void,
    InstanceOf,
    Inc,
    Dec,
    ModuloNum,
    Mul,
    Div,
    Add,
    Sub,
    StrictEqual,
    StrictUnequal,
    LooseEqual,
    LooseUnequal,
    Lesser,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    BitFlip,
    BitLShift,
    BitRShift,
    BitAnd,
    BitXor,
    BitOr,
    LogicalAnd,
    LogicalOr,
    Assign,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SyntaxId {
    Nil,
    Literal,
    ObjectExpr,
    ArrayExpr,
    Lambda,
    Lhs,
    Unary,
    Binary,
    Cond,
    Assign,
    Call,
    FuncDecl,
    Block,
    Vars,
    Ifs,
    While,
    CLikeFor,
    Break,
    Continue,
    Return,
    ExprStmt,
    EmptyStmt,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PropDeclTag {
    Data,
    Getter,
    Setter,
}

#[derive(Debug)]
pub struct PropDecl {
    pub name_tk_id: usize,
    pub initializer: Box<SyntaxNode>,
    pub tag: PropDeclTag
}

#[derive(Debug)]
pub enum SyntaxData {
    /// Stores index into token buffer, saving memory
    Nil,
    Literal(usize),
    ObjectExpr {
        props: Vec<PropDecl>,
    },
    ArrayExpr {
        items: Vec<Box<SyntaxNode>>,
    },
    Lambda {
        /// Contains indices to identifier tokens here, not the actual tokens!
        params: Vec<usize>,
        body: Box<SyntaxNode>,
    },
    Lhs {
        /// Represents a sequence of LHS property / member accesses, but the `bool`` tells if 'bracketed' access applies.
        accesses: Vec<(bool, Box<SyntaxNode>)>,
        source: Box<SyntaxNode>,
    },
    Unary {
        inner: Box<SyntaxNode>,
        op: Operator,
        prefix: bool
    },
    Binary {
        l: Box<SyntaxNode>,
        r: Box<SyntaxNode>,
        op: Operator,
    },
    Assign {
        dest: Box<SyntaxNode>,
        src: Box<SyntaxNode>,
    },
    Cond {
        check: Box<SyntaxNode>,
        l: Box<SyntaxNode>,
        r: Box<SyntaxNode>
    },
    Call {
        args: Vec<Box<SyntaxNode>>,
        callee: Box<SyntaxNode>,
    },
    FuncDecl {
        params: Vec<usize>,
        body: Box<SyntaxNode>,
        name_tk_id: usize,
    },
    Block {
        stmts: Vec<Box<SyntaxNode>>
    },
    Vars {
        /// name token index --> initializer expr
        vars: Vec<(usize, Option<Box<SyntaxNode>>)>,
    },
    Ifs {
        cond: Box<SyntaxNode>,
        t_block: Box<SyntaxNode>,
        f_block: Box<SyntaxNode>,
    },
    While {
        cond: Box<SyntaxNode>,
        body: Box<SyntaxNode>,
    },
    CLikeFor {
        init: Box<SyntaxNode>,
        cond: Box<SyntaxNode>,
        update: Box<SyntaxNode>,
        body: Box<SyntaxNode>,
    },
    Break {},
    Continue {},
    Return {
        out: Box<SyntaxNode>,
    },
    ExprStmt {
        inner: Box<SyntaxNode>,
    },
    EmptyStmt {}
}

impl SyntaxData {
    pub fn get_emitter_id(&self) -> SyntaxId {
        match self {
            Self::Nil => SyntaxId::Nil,
            Self::Literal(_) => SyntaxId::Literal,
            Self::ObjectExpr {..} => SyntaxId::ObjectExpr,
            Self::ArrayExpr {..} => SyntaxId::ArrayExpr,
            Self::Lambda {..} => SyntaxId::Lambda,
            Self::Lhs {..} => SyntaxId::Lhs,
            Self::Unary {..} => SyntaxId::Unary,
            Self::Binary {..} => SyntaxId::Binary,
            Self::Assign {..} => SyntaxId::Assign,
            Self::Cond {..} => SyntaxId::Cond,
            Self::Call {..} => SyntaxId::Call,
            Self::FuncDecl {..} => SyntaxId::FuncDecl,
            Self::Block {..} => SyntaxId::Block,
            Self::Vars {..} => SyntaxId::Vars,
            Self::Ifs {..} => SyntaxId::Ifs,
            Self::While {..} => SyntaxId::While,
            Self::CLikeFor {..} => SyntaxId::CLikeFor,
            Self::Break {} => SyntaxId::Break,
            Self::Continue {} => SyntaxId::Continue,
            Self::Return {..} => SyntaxId::Return,
            Self::ExprStmt {..} => SyntaxId::ExprStmt,
            Self::EmptyStmt {} => SyntaxId::EmptyStmt,
        }
    }
}

#[derive(Debug)]
pub struct SyntaxNode {
    pub data: SyntaxData,
    pub first_tk: usize,
    pub end_tk: usize,
}

impl SyntaxNode {
    pub fn is_empty_stmt(&self) -> bool {
        matches!(self.data, SyntaxData::EmptyStmt {})
    }
}

pub struct AST {    
    /// stores the source text string
    pub txt: String,
    /// caches tokens for AST roots
    pub tokens: Vec<Token>,
    pub decls: Vec<Box<SyntaxNode>>,
    pub name: String,
}