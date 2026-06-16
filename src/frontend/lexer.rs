use std::str::CharIndices;
use std::collections::HashMap;

use crate::frontend::{lexer::utils::DUD_SYMBOL, token::{Token, TokenKind}};

pub mod utils {
    pub const DUD_SYMBOL: char = '\0';
    pub type LexicalFn = fn(char) -> bool;
}

pub struct Lexer<'source_lt> {
    pub specials: HashMap<&'static str, TokenKind>,
    pub iter: CharIndices<'source_lt>,
    pub pos: u32,
    pub end: u32,
    pub line: u16,
    pub previous: char,
    pub peeked: char
}

impl<'source_lt> Lexer<'source_lt> {
    pub fn new(source: &'source_lt str) -> Self {
        Self {
            specials: HashMap::default(),
            iter: source.char_indices(),
            pos: 0,
            end: source.len() as u32 - 1u32,
            line: 1,
            previous: DUD_SYMBOL,
            peeked: DUD_SYMBOL
        }
    }

    pub fn map_special_lexical(&mut self, lexeme: &'static str, tag: TokenKind) {
        self.specials.insert(lexeme, tag);
    }

    const fn at_eos(&self) -> bool {
        self.pos >= self.end
    }

    fn advance(&mut self) -> char {
        if let Some((pos, symbol)) = self.iter.next() {
            self.pos = pos as u32;

            return symbol;
        }

        utils::DUD_SYMBOL
    }

    fn consume(&mut self) {
        self.previous = self.peeked;
        self.peeked = self.advance();

        if self.peeked == '\n' {
            self.line += 1;
        }
    }

    fn lex_single(&mut self, tag: TokenKind) -> Token {
        let token_begin = self.pos;
        let token_line = self.line;

        self.consume();
        let token_last = self.pos;

        Token {
            begin: token_begin,
            end: token_last,
            line: token_line,
            kind: tag
        }
    }

    fn lex_while(&mut self, predicate: utils::LexicalFn) -> Token {
        let token_begin = self.pos;
        let mut token_last = self.pos;
        let token_line = self.line;

        while self.peeked != '\0' {
            if predicate(self.peeked) {
                self.consume();
                token_last = self.pos;
            } else {
                break;
            }
        }

        Token {
            begin: token_begin,
            end: token_last,
            line: token_line,
            kind: TokenKind::Unknown
        }
    }

    fn lex_until(&mut self, predicate: utils::LexicalFn) -> Token {
        let token_begin = self.pos;
        let mut token_last = self.pos;
        let token_line = self.line;

        while self.peeked != '\0' {
            if !predicate(self.peeked) {
                self.consume();
                token_last = self.pos;
            } else {
                break;
            }
        }

        Token {
            begin: token_begin,
            end: token_last,
            line: token_line,
            kind: TokenKind::Unknown
        }
    }

    fn lex_spaces(&mut self) -> Token {
        let mut tk_temp = self.lex_while(|c| {
            c.is_whitespace()
        });

        tk_temp.kind = TokenKind::Spaces;

        tk_temp
    }

    fn lex_block_comment(&mut self) -> Token {
        self.consume(); // skip '*' of leading '*/'

        let token_begin = self.pos;
        let token_last = self.pos;
        let token_line = self.line;

        while self.peeked != '\0' {
            self.consume();

            if self.previous == '*' && self.peeked == '/' {
                break;
            }
        }

        Token {
            begin: token_begin,
            end: token_last,
            line: token_line,
            kind: TokenKind::BlockComment
        }
    }

    fn lex_slashed(&mut self) -> Token {
        let start_pos = self.pos;
        self.consume();

        if self.peeked == '/' {
            // line comments
            let mut tmp = self.lex_until(|c| {c == '\n'});
            tmp.kind = TokenKind::LineComment;
            tmp
        } else if self.peeked == '*' {
            // block comments
            self.lex_block_comment()
        } else if self.peeked.is_whitespace() {
            Token {
                begin: start_pos,
                end: self.pos,
                line: self.line,
                kind: TokenKind::OperatorSlash
            }
        } else {
            Token {
                begin: start_pos,
                end: self.pos,
                line: self.line,
                kind: TokenKind::Unknown
            }
        }
    }

    fn lex_number(&mut self) -> Token {
        let symbols: &'static str;
        let token_begin = self.pos;
        let mut token_last = self.pos;
        let token_line = self.line;
        let mut has_dot = false;
        let base_char: char;

        if self.peeked == '0' {
            self.consume();
            token_last = self.pos;

            match self.peeked {
                'b' => {
                    // binary base int
                    symbols = "01";
                    base_char = 'b';
                    self.consume();
                },
                'x' => {
                    // hex base int
                    symbols = "0123456789abcdefABCDEF";
                    base_char = 'x';
                    self.consume();
                },
                '.' => {
                    // base 10 int
                    symbols = "0123456789";
                    base_char = 'd';
                    self.consume();
                },
                'o' => {
                    // octal base int
                    symbols = "01234567";
                    base_char = 'o';
                    self.consume();
                },
                _ => {
                    // octal base via 0###
                    symbols = "01234567";
                    base_char = 'o';
                }
            }
        } else {
            symbols = "0123456789";
            base_char = 'd';
        }

        while self.peeked != '\0' {
            if symbols.contains(self.peeked) {
                self.consume();
                token_last = self.pos;
            } else if self.peeked == '.' {
                if has_dot {
                    break;
                } else {
                    has_dot = true;
                    self.consume();
                }
            } else {
                break;
            }
        }

        Token {
            begin: token_begin,
            end: token_last,
            line: token_line,
            kind: if has_dot {
                TokenKind::LiteralFloat
            } else {
                match base_char {
                    'b' => TokenKind::LiteralBinInt,
                    'x' => TokenKind::LiteralHexInt,
                    'o' => TokenKind::LiteralOctInt,
                    _ => TokenKind::LiteralDecInt
                }
            }
        }
    }

    fn lex_word(&mut self, source: &'source_lt str) -> Token {
        let mut temp_token = self.lex_while(|c| {c.is_alphanumeric()});

        let temp_lexeme = temp_token.to_str(source);
        temp_token.kind = *self.specials.get(temp_lexeme).unwrap_or(&TokenKind::Identifier);

        temp_token
    }

    fn lex_operator(&mut self, source: &'source_lt str) -> Token {
        let mut temp_token = self.lex_while(|c| {matches!(c, '%' | '*' | '/' | '+' | '-' | '!' | '<' | '>' | '=' | '&' | '|' | '^' | '~')});

        let temp_lexeme = temp_token.to_str(source);
        temp_token.kind = *self.specials.get(temp_lexeme).unwrap_or(&TokenKind::Unknown);

        temp_token
    }

    pub fn lex_all(&mut self, source: &'source_lt str) -> Vec<Token> {
        let mut tokens = Vec::<Token>::default();

        self.consume();

        while !self.at_eos() {
            let temp = match self.peeked {
                '/' => self.lex_slashed(),
                ',' => self.lex_single(TokenKind::Comma),
                '.' => self.lex_single(TokenKind::Dot),
                ':' => self.lex_single(TokenKind::Colon),
                ';' => self.lex_single(TokenKind::Semicolon),
                '(' => self.lex_single(TokenKind::LeftParen),
                ')' => self.lex_single(TokenKind::RightParen),
                '{' => self.lex_single(TokenKind::LeftBrace),
                '}' => self.lex_single(TokenKind::RightBrace),
                '[' => self.lex_single(TokenKind::LeftBracket),
                ']' => self.lex_single(TokenKind::RightBracket),
                '?' => self.lex_single(TokenKind::QMark),
                _ => if self.peeked.is_whitespace() {
                    self.lex_spaces()
                } else if self.peeked.is_ascii_digit() {
                    self.lex_number()
                } else if self.peeked.is_alphabetic() {
                    self.lex_word(source)
                } else if matches!(self.peeked, '%' | '*' | '/' | '+' | '-' | '!' | '<' | '>' | '=' | '&' | '|' | '^' | '~') {
                    self.lex_operator(source)
                } else {
                    self.lex_single(TokenKind::Unknown)
                }
            };

            if matches!(temp.kind, TokenKind::Spaces | TokenKind::LineComment | TokenKind::BlockComment) {
                continue;
            }

            tokens.push(temp);
        }

        tokens
    }
}