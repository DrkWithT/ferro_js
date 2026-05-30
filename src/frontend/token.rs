use std::fmt::Display;

#[allow(unused)]
macro_rules! MATCH_TKIND {
    ($tk_name: ident, $first: literal) => {
        ($tk_name.kind == $first)
    };
    ($tk_name: ident, $first: literal, $rest: literal+) => {
        ($tk_name.kind == $first) || MATCH_TKIND($tk_name, $rest)
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TokenKind {
    Unknown,
    Spaces,
    LineComment,
    BlockComment,
    KeywordVar,
    KeywordIf,
    KeywordElse,
    KeywordWhile,
    KeywordFor,
    KeywordReturn,
    KeywordFunction,
    KeywordGet,
    KeywordSet,
    KeywordTypeOf,
    KeywordDelete,
    Identifier,
    OperatorPlusPlus,
    OperatorMinusMinus,
    OperatorBang,
    OperatorPct,
    OperatorTimes,
    OperatorSlash,
    OperatorPlus,
    OperatorMinus,
    OperatorStrictEquals,
    OperatorStrictUnequals,
    OperatorLesser,
    OperatorLesserEquals,
    OperatorGreater,
    OperatorGreaterEquals,
    OperatorAnd,
    OperatorOr,
    OperatorAssign,
    OperatorBitOr,
    OperatorBitAnd,
    OperatorBShiftLeft,
    OperatorBShiftRight,
    LiteralNull,
    LiteralUndefined,
    LiteralNaN,
    LiteralTrue,
    LiteralFalse,
    LiteralDecInt,
    LiteralHexInt,
    LiteralBinInt,
    LiteralOctInt,
    LiteralFloat,
    LiteralString,
    Comma,
    Colon,
    Dot,
    Semicolon,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Eof
}

impl Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match *self {
            Self::Unknown => "(?)",
            Self::Spaces => "(spaces)",
            Self::LineComment => "(line-comment)",
            Self::BlockComment => "(block-comment)",
            Self::KeywordVar => "var",
            Self::KeywordIf => "if",
            Self::KeywordElse => "else",
            Self::KeywordWhile => "while",
            Self::KeywordFor => "for",
            Self::KeywordReturn => "return",
            Self::KeywordFunction => "function",
            Self::KeywordGet => "get",
            Self::KeywordSet => "set",
            Self::KeywordTypeOf => "typeof",
            Self::KeywordDelete => "delete",
            Self::Identifier => "(name)",
            Self::OperatorPlusPlus => "++",
            Self::OperatorMinusMinus => "--",
            Self::OperatorBang => "!",
            Self::OperatorPct => "%",
            Self::OperatorTimes => "*",
            Self::OperatorSlash => "/",
            Self::OperatorPlus => "+",
            Self::OperatorMinus => "-",
            Self::OperatorStrictEquals => "===",
            Self::OperatorStrictUnequals => "!==",
            Self::OperatorLesser => "<",
            Self::OperatorLesserEquals => "<=",
            Self::OperatorGreater => ">",
            Self::OperatorGreaterEquals => ">=",
            Self::OperatorAnd => "&&",
            Self::OperatorOr => "||",
            Self::OperatorAssign => "=",
            Self::OperatorBitOr => "|",
            Self::OperatorBitAnd => "&",
            Self::OperatorBShiftLeft => "<<",
            Self::OperatorBShiftRight => ">>",
            Self::LiteralNull => "null",
            Self::LiteralUndefined => "undefined",
            Self::LiteralNaN => "NaN",
            Self::LiteralTrue => "true",
            Self::LiteralFalse => "false",
            Self::LiteralDecInt => "(literal-dec-int)",
            Self::LiteralHexInt => "(literal-hex-int)",
            Self::LiteralBinInt => "(literal-bin-int)",
            Self::LiteralOctInt => "(literal-oct-int)",
            Self::LiteralFloat => "(number-float)",
            Self::LiteralString => "(literal-string)",
            Self::Comma => ",",
            Self::Colon => ":",
            Self::Dot => ".",
            Self::Semicolon => ";",
            Self::LeftParen => "(",
            Self::RightParen => ")",
            Self::LeftBrace => "{",
            Self::RightBrace => "}",
            Self::LeftBracket => "[",
            Self::RightBracket => "]",
            Self::Eof => "(EOF)"
        })
    }
}

pub struct Token {
    pub begin: u32,
    pub end: u32,
    pub line: u16,
    pub kind: TokenKind
}

impl Token {
    pub fn eof(start: u32) -> Self {
        Self {
            begin: start,
            end: start,
            line: 0,
            kind: TokenKind::Eof
        }
    }

    pub fn to_str<'source_lt>(&'source_lt self, source: &'source_lt str) -> &'source_lt str {
        &source[self.begin as usize .. self.end as usize]
    }

    pub fn to_string(&self, source: &str) -> String {
        let tmp = &source[self.begin as usize .. self.end as usize];

        tmp.to_owned()
    }
}
