
use crate::frontend::token::Token;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operator {
    NegBool,
    ForceNum,
    NegNum,
    ModuloNum,
    MulNum,
    DivNum,
    AddNum,
    SubNum,
    StrictEqual,
    StrictUnequal,
    Lesser,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    LogicalAnd,
    LogicalOr,
    Assign,
}

#[repr(u8)]
pub enum SyntaxId {
    Literal,
    Unary,
    Binary,
    Access,
    Assign,
    Call,
    FuncDecl,
    Block,
    Vars,
    Ifs,
    While,
    CLikeFor,
    Return,
    ExprStmt,
}

pub enum SyntaxData {
    /// Stores index into token buffer, saving memory
    Literal(usize),
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
    Access {
        parent: Box<SyntaxNode>,
        key: Box<SyntaxNode>,
        is_named: bool
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
        args: Vec<Box<SyntaxNode>>,
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
    Return {
        out: Box<SyntaxNode>,
    },
    ExprStmt {
        inner: Box<SyntaxNode>,
    }
}

impl SyntaxData {
    pub fn get_emitter_id(self) -> SyntaxId {
        match self {
            Self::Literal(_) => SyntaxId::Literal,
            Self::Unary {..} => SyntaxId::Unary,
            Self::Binary {..} => SyntaxId::Binary,
            Self::Access {..} => SyntaxId::Access,
            Self::Assign {..} => SyntaxId::Assign,
            Self::Call {..} => SyntaxId::Call,
            Self::FuncDecl {..} => SyntaxId::FuncDecl,
            Self::Block {..} => SyntaxId::Block,
            Self::Vars {..} => SyntaxId::Vars,
            Self::Ifs {..} => SyntaxId::Ifs,
            Self::While {..} => SyntaxId::While,
            Self::CLikeFor {..} => SyntaxId::CLikeFor,
            Self::Return {..} => SyntaxId::Return,
            Self::ExprStmt {..} => SyntaxId::ExprStmt,
        }
    }
}

pub struct SyntaxNode {
    pub data: SyntaxData,
    pub first_tk: usize,
    pub end_tk: usize,
}

pub struct AST {
    /// stores the source file's path
    pub file_path: String,    
    /// stores the source text string
    pub txt: String,
    /// caches tokens for AST roots
    pub tokens: Vec<Token>,
}