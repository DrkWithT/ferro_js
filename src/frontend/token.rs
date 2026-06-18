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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    KeywordNew,
    KeywordThis,
    KeywordGet,
    KeywordSet,
    KeywordTypeOf,
    KeywordVoid,
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
    OperatorLooseEqual,
    OperatorLooseUnequal,
    OperatorLesser,
    OperatorLesserEquals,
    OperatorGreater,
    OperatorGreaterEquals,
    OperatorAnd,
    OperatorOr,
    OperatorAssign,
    OperatorBitFlip,
    OperatorBitAnd,
    OperatorBitXor,
    OperatorBitOr,
    OperatorBShiftLeft,
    OperatorBShiftRight,
    LiteralNull,
    LiteralUndefined,
    LiteralNaN,
    LiteralInfinity,
    LiteralTrue,
    LiteralFalse,
    LiteralDecInt,
    LiteralHexInt,
    LiteralBinInt,
    LiteralOctInt,
    LiteralFloat,
    LiteralString,
    LiteralEscapedString,
    Comma,
    Colon,
    Dot,
    QMark,      // `?` for conditional operator
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
            Self::KeywordNew => "new",
            Self::KeywordThis => "this",
            Self::KeywordGet => "get",
            Self::KeywordSet => "set",
            Self::KeywordTypeOf => "typeof",
            Self::KeywordVoid => "void",
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
            Self::OperatorLooseEqual => "==",
            Self::OperatorLooseUnequal => "!=",
            Self::OperatorLesser => "<",
            Self::OperatorLesserEquals => "<=",
            Self::OperatorGreater => ">",
            Self::OperatorGreaterEquals => ">=",
            Self::OperatorAnd => "&&",
            Self::OperatorOr => "||",
            Self::OperatorAssign => "=",
            Self::OperatorBitFlip => "~",
            Self::OperatorBitOr => "|",
            Self::OperatorBitAnd => "&",
            Self::OperatorBitXor => "^",
            Self::OperatorBShiftLeft => "<<",
            Self::OperatorBShiftRight => ">>",
            Self::LiteralNull => "null",
            Self::LiteralUndefined => "undefined",
            Self::LiteralNaN => "NaN",
            Self::LiteralInfinity => "Infinity",
            Self::LiteralTrue => "true",
            Self::LiteralFalse => "false",
            Self::LiteralDecInt => "(literal-dec-int)",
            Self::LiteralHexInt => "(literal-hex-int)",
            Self::LiteralBinInt => "(literal-bin-int)",
            Self::LiteralOctInt => "(literal-oct-int)",
            Self::LiteralFloat => "(number-float)",
            Self::LiteralString => "(literal-string)",
            Self::LiteralEscapedString => "(literal-escaped-string)",
            Self::Comma => ",",
            Self::Colon => ":",
            Self::Dot => ".",
            Self::QMark => "?",
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

#[derive(Clone)]
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

    pub fn to_unescaped_string(&self, source: &str) -> String {
        const HEX_DIGIT_0: u8 = '0'.to_ascii_lowercase() as u8;
        const HEX_LTR_0: u8 = 'A'.to_ascii_lowercase() as u8;

        if self.kind != TokenKind::LiteralEscapedString {
            return self.to_str(source).to_owned();
        }

        let lexeme = &source[self.begin as usize .. self.end as usize];

        #[repr(u8)]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        enum DEscState {
            Check,
            Regular,
            Char1,
            Char2,
            Ascii,
            Done,
        }

        let mut state = DEscState::Check;
        let chars = lexeme.chars().collect::<Vec<char>>();
        let mut s = String::default();
        let mut spos: usize = 0;
        let mut hex_low = 0u8;
        let mut hex_high = 0u8;

        while state != DEscState::Done {
            let c = *chars.get(spos).unwrap_or(&'\0');

            state = match state {
                DEscState::Check => {
                    if c == '\\' { spos += 1; DEscState::Char1 } else if c != '\0' { DEscState::Regular } else { DEscState::Done }
                },
                DEscState::Regular => {
                    s.push(c);
                    spos += 1;
                    DEscState::Check
                },
                DEscState::Char1 => {
                    if matches!(c, 't' | 'r' | 'n' | '\\' | '\'' | '\"') {
                        s.push(c);
                        spos += 1;
                        DEscState::Check
                    } else if c == 'x' {
                        spos += 1;
                        DEscState::Char1
                    } else if c.is_ascii_digit() {
                        hex_high = c as u8 - HEX_DIGIT_0;
                        spos += 1;
                        DEscState::Char2
                    } else if c.is_ascii_hexdigit() {
                        hex_high = c.to_ascii_lowercase() as u8 - HEX_LTR_0;
                        spos += 1;
                        DEscState::Char2
                    } else {
                        spos += 1;
                        DEscState::Check
                    }
                },
                DEscState::Char2 => {
                    if c.is_ascii_digit() {
                        hex_low = c.to_ascii_lowercase() as u8 - HEX_DIGIT_0;
                        spos += 1;
                        DEscState::Ascii
                    } else if c.is_ascii_hexdigit() {
                        hex_low = c.to_ascii_lowercase() as u8 - HEX_LTR_0;
                        spos += 1;
                        DEscState::Ascii
                    } else {
                        spos += 1;
                        DEscState::Check
                    }
                },
                DEscState::Ascii => {
                    s.push((hex_high * 16 + hex_low) as char);
                    DEscState::Check
                },
                _ => state,
            };
        }

        s
    }
}
