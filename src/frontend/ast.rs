
use crate::frontend::token::Token;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    Lesser,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    BitAnd,
    BitOr,
    LogicalAnd,
    LogicalOr,
    Assign,
}

#[repr(u8)]
pub enum SyntaxId {
    Literal,
    ObjectExpr,
    ArrayExpr,
    Lambda,
    Lhs,
    Unary,
    Binary,
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

pub enum SyntaxData {
    /// Stores index into token buffer, saving memory
    Literal(usize),
    ObjectExpr {
        props: Vec<(usize, Box<SyntaxNode>)>,
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
        vars: Vec<(usize, Box<SyntaxNode>)>,
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
    pub fn get_emitter_id(self) -> SyntaxId {
        match self {
            Self::Literal(_) => SyntaxId::Literal,
            Self::ObjectExpr { .. } => SyntaxId::ObjectExpr,
            Self::ArrayExpr {..} => SyntaxId::ArrayExpr,
            Self::Lambda { .. } => SyntaxId::Lambda,
            Self::Lhs {..} => SyntaxId::Lhs,
            Self::Unary {..} => SyntaxId::Unary,
            Self::Binary {..} => SyntaxId::Binary,
            Self::Assign {..} => SyntaxId::Assign,
            Self::Call {..} => SyntaxId::Call,
            Self::FuncDecl {..} => SyntaxId::FuncDecl,
            Self::Block {..} => SyntaxId::Block,
            Self::Vars {..} => SyntaxId::Vars,
            Self::Ifs {..} => SyntaxId::Ifs,
            Self::While {..} => SyntaxId::While,
            Self::CLikeFor {..} => SyntaxId::CLikeFor,
            Self::Break {  } => SyntaxId::Break,
            Self::Continue {  } => SyntaxId::Continue,
            Self::Return {..} => SyntaxId::Return,
            Self::ExprStmt {..} => SyntaxId::ExprStmt,
            Self::EmptyStmt {} => SyntaxId::EmptyStmt,
        }
    }
}

pub struct SyntaxNode {
    pub data: SyntaxData,
    pub first_tk: usize,
    pub end_tk: usize,
}

pub struct AST {    
    /// stores the source text string
    pub txt: String,
    /// caches tokens for AST roots
    pub tokens: Vec<Token>,
    pub decls: Vec<Box<SyntaxNode>>
}