use crate::frontend::{
    ast::{Operator, SyntaxData, SyntaxNode},
    token::{Token, TokenKind}
};

macro_rules! TOKEN_KIND_IS {
    ($token: expr) => {
        false
    };
    ($token: expr, $first_tag: path) => {
        $token.kind == $first_tag
    };
    ($token: expr, $first_tag: path, $($rest: path)*) => {
        $token.kind == $first_tag || TOKEN_KIND_IS($token, $rest)
    };
}

macro_rules! CONSUME_OF {
    ($consumer: expr, $err: expr, $token: expr, $tag: path) => {
        if TOKEN_KIND_IS!($token, $tag) {
            $consumer;
        } else {
            return Err($err);
        }
    };
    ($consumer: expr, $err: expr, $token: expr, $tags: path|+) => {
        if TOKEN_KIND_IS($token, $tags) {
            $consumer;
        } else {
            return Err(err);
        }
    };
}

#[allow(dead_code)]
#[derive(Clone)]
struct ParseErr {
    pub culprit: Token,
    pub msg: &'static str,
    pub line: u16,
}

pub struct Parser<'extern_src_lt> {
    pub tokens: &'extern_src_lt [Token],
    pub text: &'extern_src_lt str,
    pub pos: usize,
    pub errors: u32,
    pub line: u16,
}

impl<'external_content_lt> Parser<'external_content_lt> {
    pub fn new(tokens: &'external_content_lt [Token], text: &'external_content_lt str) -> Self {
        Self {
            tokens,
            text,
            pos: 0,
            errors: 0,
            line: 1
        }
    }

    fn at_eof(&self) -> bool {
        self.pos >= self.tokens.len() || self.tokens[self.pos].kind == TokenKind::Eof
    }

    fn consume(&mut self) {
        if !self.at_eof() {   
            self.pos += 1;
        }
    }

    fn recover(&mut self) {
        while !self.at_eof() {
            if self.tokens[self.pos].kind == TokenKind::KeywordFunction {
                break;
            }
            self.consume();
        }
    }

    fn parse_primary(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let tk_pos = self.pos;
        let current = &self.tokens[self.pos];

        if matches!(current.kind, TokenKind::LiteralUndefined | TokenKind::LiteralNull | TokenKind::LiteralNaN | TokenKind::LiteralTrue | TokenKind::LiteralFalse | TokenKind::LiteralDecInt | TokenKind::LiteralHexInt | TokenKind::LiteralBinInt | TokenKind::LiteralOctInt | TokenKind::LiteralFloat | TokenKind::LiteralString | TokenKind::KeywordThis | TokenKind::Identifier) {
            self.consume();
            return Ok(Box::new(SyntaxNode {
                data: SyntaxData::Literal(tk_pos),
                first_tk: tk_pos,
                end_tk: tk_pos + 1,
            }));
        } else if current.kind == TokenKind::LeftBracket {
            return self.parse_array();
        } else if current.kind == TokenKind::LeftParen {
            self.consume();

            let wrapped_expr = self.parse_or()?;

            let rparen_tk = &self.tokens[self.pos];
            CONSUME_OF!(self.consume(), ParseErr { culprit: rparen_tk.clone(), msg: "Expected ')' ending wrapped expr.", line: rparen_tk.line }, rparen_tk.clone(), TokenKind::RightParen);

            return Ok(wrapped_expr);
        } else if current.kind == TokenKind::KeywordFunction {
            return self.parse_lambda();
        }

        Err(ParseErr { culprit: current.clone(), msg: "Unexpected token for JS primitive literal.", line: current.line })
    }

    #[allow(unused)]
    fn parse_object(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        // todo: implement later
        Err(ParseErr {culprit: Token::eof(0), msg: "Not implemented!", line: 0})
    }

    fn parse_array(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {        
        let first_tk_pos = self.pos;
        self.consume(); // ? skip '['
        
        let mut items = Vec::<Box<SyntaxNode>>::default();

        if self.tokens[self.pos].kind != TokenKind::RightBracket {
            items.push(self.parse_or()?);
        }

        while !self.at_eof() {
            if self.tokens[self.pos].kind != TokenKind::Comma {
                break;
            }
            self.consume();

            items.push(self.parse_or()?);
        }

        let ending_tk = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr { culprit: ending_tk.clone(), msg: "Expected ']' ending array literal.", line: ending_tk.line }, ending_tk.clone(), TokenKind::RightBracket);

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::ArrayExpr { items },
            first_tk: first_tk_pos,
            end_tk: self.pos
        }))
    }

    fn parse_lambda(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;
        self.consume(); // ? skip 'function'

        let lparen = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr {culprit: lparen.clone(), msg: "Expected '(' beginning lambda params.", line: lparen.line}, lparen.clone(), TokenKind::LeftParen);

        let mut params = Vec::<usize>::default();

        if self.tokens[self.pos].kind != TokenKind::RightParen {
            let first_tk = &self.tokens[self.pos];
            CONSUME_OF!(self.consume(), ParseErr { culprit: first_tk.clone(), msg: "Expected identifier for 1st lambda param.", line: first_tk.line }, first_tk.clone(), TokenKind::Identifier);
            params.push(self.pos - 1);
        }

        while !self.at_eof() {
            if self.tokens[self.pos].kind != TokenKind::Comma {
                break;
            }
            self.consume();

            let name_tk = &self.tokens[self.pos];
            CONSUME_OF!(self.consume(), ParseErr { culprit: name_tk.clone(), msg: "Expected name in lambda param.", line: name_tk.line }, name_tk.clone(), TokenKind::Identifier);
            params.push(self.pos - 1);
        }

        let ending_tk = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr { culprit: ending_tk.clone(), msg: "Expected ')' ending lambda params.", line: ending_tk.line }, ending_tk.clone(), TokenKind::RightParen);

        let body = self.parse_block()?;

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::Lambda { params, body },
            first_tk: first_tk_pos,
            end_tk: self.pos
        }))
    }

    fn parse_lhs(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;

        let source = self.parse_primary()?;

        if !matches!(self.tokens[self.pos].kind, TokenKind::Dot | TokenKind::LeftBracket) {
            return Ok(source);
        }

        let mut accesses = Vec::<(bool, Box<SyntaxNode>)>::default();

        while !self.at_eof() {
            let access_symbol = &self.tokens[self.pos];

            match access_symbol.kind {
                TokenKind::Dot => {
                    self.consume();

                    let name_tk = &self.tokens[self.pos];
                    CONSUME_OF!(self.consume(), ParseErr { culprit: name_tk.clone(), msg: "Expected property name after '.' in lhs-expr.", line: name_tk.line }, name_tk.clone(), TokenKind::Identifier);

                    accesses.push((false, Box::new(SyntaxNode {
                        data: SyntaxData::Literal(self.pos - 1),
                        first_tk: self.pos - 1,
                        end_tk: self.pos
                    })));
                },
                TokenKind::LeftBracket => {
                    self.consume();

                    let key_expr = self.parse_or()?;

                    let rbrack_tk: &Token = &self.tokens[self.pos];
                    CONSUME_OF!(self.consume(), ParseErr { culprit: rbrack_tk.clone(), msg: "Expected ']' closing a key access in lhs-expr.", line: rbrack_tk.line }, rbrack_tk.clone(), TokenKind::RightBracket);

                    accesses.push((true, key_expr));
                },
                _ => {
                    break;
                }
            }
        }

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::Lhs { accesses, source },
            first_tk: first_tk_pos,
            end_tk: self.pos
        }))
    }

    fn parse_new(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;

        if self.tokens[first_tk_pos].kind != TokenKind::KeywordNew {
            return self.parse_lhs();
        }
        self.consume();

        let inner = self.parse_lhs()?;

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::Unary {
                inner,
                op: Operator::New,
                prefix: true,
            },
            first_tk: first_tk_pos,
            end_tk: self.pos
        }))
    }

    fn parse_call(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;
        let callee = self.parse_new()?;
        
        if self.tokens[self.pos].kind != TokenKind::LeftParen {
            return Ok(callee);
        }
        self.consume();

        let mut args = Vec::<Box<SyntaxNode>>::default();

        if self.tokens[self.pos].kind != TokenKind::RightParen {
            args.push(self.parse_or()?);
        }

        while !self.at_eof() {
            if self.tokens[self.pos].kind != TokenKind::Comma {
                break;
            }
            self.consume();

            args.push(self.parse_or()?);
        }

        let rparen_tk = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr { culprit: rparen_tk.clone(), msg: "Expected ')' closing call-expr args.", line: rparen_tk.line }, rparen_tk.clone(), TokenKind::RightParen);

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::Call { args, callee },
            first_tk: first_tk_pos,
            end_tk: self.pos
        }))
    }

    fn parse_postfix_unary(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;
        let inner = self.parse_call()?;

        let postfix_unary_op = match self.tokens[self.pos].kind {
            TokenKind::OperatorPlusPlus => {
                self.consume();
                Operator::Inc
            },
            TokenKind::OperatorMinusMinus => {
                self.consume();
                Operator::Dec
            },
            _ => Operator::Noop,
        };

        if postfix_unary_op == Operator::Noop {
            return Ok(inner);
        }

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::Unary { inner, op: postfix_unary_op, prefix: false },
            first_tk: first_tk_pos,
            end_tk: self.pos
        }))
    }

    fn parse_prefix_unary(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;

        let prefix_unary_op = match self.tokens[self.pos].kind {
            TokenKind::KeywordNew => Operator::New,
            TokenKind::OperatorBang => Operator::NegBool,
            TokenKind::OperatorPlus => Operator::ForceNum,
            TokenKind::OperatorPlusPlus => Operator::Inc,
            TokenKind::OperatorMinusMinus => Operator::Dec,
            TokenKind::KeywordTypeOf => Operator::TypeOf,
            TokenKind::KeywordVoid => Operator::Void,
            _ => Operator::Noop,
        };

        if prefix_unary_op != Operator::Noop {
            self.consume();
        } else {
            return self.parse_postfix_unary();
        }

        let inner = self.parse_postfix_unary()?;

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::Unary { inner, op: prefix_unary_op, prefix: true },
            first_tk: first_tk_pos,
            end_tk: self.pos
        }))
    }

    fn parse_factor(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let mut lhs = self.parse_prefix_unary()?;

        while !self.at_eof() {
            let factor_op = match self.tokens[self.pos].kind {
                TokenKind::OperatorPct => Operator::ModuloNum,
                TokenKind::OperatorTimes => Operator::Mul,
                TokenKind::OperatorSlash => Operator::Div,
                _ => Operator::Noop,
            };

            if factor_op == Operator::Noop {
                break;
            }
            self.consume();

            let pre_rhs_pos = self.pos;
            lhs = Box::new(SyntaxNode {
                data: SyntaxData::Binary { l: lhs, r: self.parse_prefix_unary()?, op: factor_op },
                first_tk: pre_rhs_pos,
                end_tk: self.pos,
            });
        }

        Ok(lhs)
    }

    fn parse_term(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let mut lhs = self.parse_factor()?;

        while !self.at_eof() {
            let term_op = match self.tokens[self.pos].kind {
                TokenKind::OperatorPlus => Operator::Add,
                TokenKind::OperatorMinus => Operator::Sub,
                _ => Operator::Noop,
            };

            if term_op == Operator::Noop {
                break;
            }
            self.consume();

            let pre_rhs_pos = self.pos;
            lhs = Box::new(SyntaxNode {
                data: SyntaxData::Binary { l: lhs, r: self.parse_factor()?, op: term_op },
                first_tk: pre_rhs_pos,
                end_tk: self.pos,
            });
        }

        Ok(lhs)
    }

    fn parse_compare(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let mut lhs = self.parse_term()?;

        while !self.at_eof() {
            let compare_op = match self.tokens[self.pos].kind {
                TokenKind::OperatorLesser => Operator::Lesser,
                TokenKind::OperatorLesserEquals => Operator::LessOrEqual,
                TokenKind::OperatorGreater => Operator::Greater,
                TokenKind::OperatorGreaterEquals => Operator::GreaterOrEqual,
                _ => Operator::Noop,
            };

            if compare_op == Operator::Noop {
                break;
            }
            self.consume();

            let pre_rhs_pos = self.pos;
            lhs = Box::new(SyntaxNode {
                data: SyntaxData::Binary { l: lhs, r: self.parse_term()?, op: compare_op },
                first_tk: pre_rhs_pos,
                end_tk: self.pos,
            });
        }

        Ok(lhs)
    }

    fn parse_equality(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let mut lhs = self.parse_compare()?;

        while !self.at_eof() {
            let equality_op = match self.tokens[self.pos].kind {
                TokenKind::OperatorStrictEquals => Operator::StrictEqual,
                TokenKind::OperatorStrictUnequals => Operator::StrictUnequal,
                _ => Operator::Noop,
            };

            if equality_op == Operator::Noop {
                break;
            }
            self.consume();

            let pre_rhs_pos = self.pos;
            lhs = Box::new(SyntaxNode {
                data: SyntaxData::Binary { l: lhs, r: self.parse_compare()?, op: equality_op },
                first_tk: pre_rhs_pos,
                end_tk: self.pos,
            });
        }

        Ok(lhs)
    }

    fn parse_bit_and(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let mut lhs = self.parse_equality()?;

        while !self.at_eof() {
            if self.tokens[self.pos].kind != TokenKind::OperatorBitAnd {
                break;
            };
            self.consume();

            let pre_rhs_pos = self.pos;
            lhs = Box::new(SyntaxNode {
                data: SyntaxData::Binary { l: lhs, r: self.parse_equality()?, op: Operator::BitAnd },
                first_tk: pre_rhs_pos,
                end_tk: self.pos,
            });
        }

        Ok(lhs)
    }

    fn parse_bit_or(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let mut lhs = self.parse_bit_and()?;

        while !self.at_eof() {
            if self.tokens[self.pos].kind != TokenKind::OperatorBitOr {
                break;
            };
            self.consume();

            let pre_rhs_pos = self.pos;
            lhs = Box::new(SyntaxNode {
                data: SyntaxData::Binary { l: lhs, r: self.parse_bit_and()?, op: Operator::BitOr },
                first_tk: pre_rhs_pos,
                end_tk: self.pos,
            });
        }

        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let mut lhs = self.parse_bit_or()?;

        while !self.at_eof() {
            if self.tokens[self.pos].kind != TokenKind::OperatorAnd {
                break;
            }
            self.consume();

            let pre_rhs_pos = self.pos;
            lhs = Box::new(SyntaxNode {
                data: SyntaxData::Binary { l: lhs, r: self.parse_bit_or()?, op: Operator::LogicalAnd },
                first_tk: pre_rhs_pos,
                end_tk: self.pos,
            });
        }

        Ok(lhs)
    }

    fn parse_or(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let mut lhs = self.parse_and()?;

        while !self.at_eof() {
            if self.tokens[self.pos].kind != TokenKind::OperatorOr {
                break;
            }
            self.consume();

            let pre_rhs_pos = self.pos;
            lhs = Box::new(SyntaxNode {
                data: SyntaxData::Binary { l: lhs, r: self.parse_and()?, op: Operator::LogicalOr },
                first_tk: pre_rhs_pos,
                end_tk: self.pos,
            });
        }

        Ok(lhs)
    }



    fn parse_stmt(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        match self.tokens[self.pos].kind {
            TokenKind::KeywordVar => self.parse_vars(),
            TokenKind::KeywordIf => self.parse_if(),
            TokenKind::KeywordReturn => self.parse_return(),
            // TokenKind::KeywordBreak => self.parse_break(),
            // TokenKind::KeywordContinue => self.parse_continue(),
            TokenKind::KeywordWhile => self.parse_while(),
            // TokenKind::KeywordDo => self.parse_do_while(),
            // TokenKind::KeywordFor => self.parse_c_for_loop(),
            TokenKind::KeywordFunction => self.parse_function(),
            TokenKind::LeftBrace => self.parse_block(),
            TokenKind::Semicolon => {self.consume(); Ok(Box::new(SyntaxNode {
                data: SyntaxData::EmptyStmt {},
                first_tk: self.pos - 1,
                end_tk: self.pos,
            }))},
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_var_decl(&mut self) -> Option<(usize, Option<Box<SyntaxNode>>)> {
        let first_tk_pos = self.pos;

        let name_tk = &self.tokens[first_tk_pos];
        if name_tk.kind != TokenKind::Identifier {
            return None;
        }
        self.consume();

        let equals_tk = &self.tokens[self.pos];
        if equals_tk.kind != TokenKind::OperatorAssign {
            // ? Handle variable without initializer expr here, so just put None if no '=' is peeked.
            return Some((first_tk_pos, None));
        }
        self.consume();

        let initializer = self.parse_or().ok()?;

        Some((first_tk_pos, Some(initializer)))
    }

    fn parse_vars(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;
        self.consume(); // ? skip 'var'

        let mut var_decls = Vec::<(usize, Option<Box<SyntaxNode>>)>::default();

        if let Some(first_var) = self.parse_var_decl() {
            var_decls.push(first_var);
        } else {
            return Err(ParseErr { culprit: self.tokens[self.pos - 1].clone(), msg: "Invalid var-item #1 here.", line: self.tokens[self.pos - 1].line });
        }

        while !self.at_eof() {
            if self.tokens[self.pos].kind != TokenKind::Comma {
                break;
            }
            self.consume();

            if let Some(next_var) = self.parse_var_decl() {
                var_decls.push(next_var);
            } else {
                return Err(ParseErr { culprit: self.tokens[self.pos - 1].clone(), msg: "Invalid var-item around here.", line: self.tokens[self.pos - 1].line });
            }
        }

        let ending_tk = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr { culprit: ending_tk.clone(), msg: "Expected ';' ending var-statement.", line: ending_tk.line }, ending_tk.clone(), TokenKind::Semicolon);

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::Vars { vars: var_decls },
            first_tk: first_tk_pos,
            end_tk: self.pos
        }))
    }

    fn parse_if(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;
        self.consume(); // ? skip 'if'

        let lparen_tk = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr { culprit: lparen_tk.clone(), msg: "Expected '(' opening an if-condition here.", line: lparen_tk.line }, lparen_tk.clone(), TokenKind::LeftParen);

        let cond = self.parse_or()?;

        let rparen_tk = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr { culprit: rparen_tk.clone(), msg: "Expected '(' closing an if-condition here.", line: rparen_tk.line }, rparen_tk.clone(), TokenKind::RightParen);

        let tbody = self.parse_stmt();
        let fbody: Result<Box<SyntaxNode>, ParseErr> = if self.tokens[self.pos].kind == TokenKind::KeywordElse {
            self.consume();
            self.parse_stmt()
        } else {
            Ok(Box::new(SyntaxNode {
                data: SyntaxData::EmptyStmt {},
                first_tk: self.pos,
                end_tk: self.pos,
            }))
        };

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::Ifs {
                cond,
                t_block: tbody?,
                f_block: fbody?,
            },
            first_tk: first_tk_pos,
            end_tk: self.pos,
        }))
    }

    #[allow(unused)]
    fn parse_while(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        // todo: implement!
        Err(ParseErr {culprit: Token::eof(0), msg: "Testing!", line: 0})
    }

    #[allow(unused)]
    fn parse_c_for_loop(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        // todo: implement!
        Err(ParseErr {culprit: Token::eof(0), msg: "Testing!", line: 0})
    }

    #[allow(unused)]
    fn parse_break(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        // todo: implement!
        Err(ParseErr {culprit: Token::eof(0), msg: "Testing!", line: 0})
    }

    #[allow(unused)]
    fn parse_continue(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        // todo: implement!
        Err(ParseErr {culprit: Token::eof(0), msg: "Testing!", line: 0})
    }

    fn parse_function(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;
        self.consume(); // ? skip 'function'

        let fn_name_pos = self.pos;
        let fn_name_tk = &self.tokens[fn_name_pos];
        CONSUME_OF!(self.consume(), ParseErr { culprit: fn_name_tk.clone(), msg: "Expected name in function-decl.", line: fn_name_tk.line }, fn_name_tk.clone(), TokenKind::Identifier);

        let lparen = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr {culprit: lparen.clone(), msg: "Expected '(' beginning function params.", line: lparen.line}, lparen.clone(), TokenKind::LeftParen);

        let mut params = Vec::<usize>::default();

        if self.tokens[self.pos].kind != TokenKind::RightParen {
            let first_tk = &self.tokens[self.pos];
            CONSUME_OF!(self.consume(), ParseErr { culprit: first_tk.clone(), msg: "Expected identifier for 1st function param.", line: first_tk.line }, first_tk.clone(), TokenKind::Identifier);
            params.push(self.pos - 1);
        }

        while !self.at_eof() {
            if self.tokens[self.pos].kind != TokenKind::Comma {
                break;
            }
            self.consume();

            let name_tk = &self.tokens[self.pos];
            CONSUME_OF!(self.consume(), ParseErr { culprit: name_tk.clone(), msg: "Expected name in function param.", line: name_tk.line }, name_tk.clone(), TokenKind::Identifier);
            params.push(self.pos - 1);
        }

        let ending_tk = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr { culprit: ending_tk.clone(), msg: "Expected ')' ending lambda params.", line: ending_tk.line }, ending_tk.clone(), TokenKind::RightParen);

        let body = self.parse_block()?;

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::FuncDecl {
                params,
                body,
                name_tk_id: fn_name_pos
            },
            first_tk: first_tk_pos,
            end_tk: self.pos
        }))
    }

    fn parse_block(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;

        let opening_brace_tk = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr { culprit: opening_brace_tk.clone(), msg: "Expected '{' starting a block here.", line: opening_brace_tk.line }, opening_brace_tk.clone(), TokenKind::LeftBrace);

        let mut stmts = Vec::<Box<SyntaxNode>>::default();

        while !self.at_eof() {
            if self.tokens[self.pos].kind == TokenKind::RightBrace {
                self.consume();
                break;
            }

            stmts.push(self.parse_stmt()?);
        }

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::Block { stmts },
            first_tk: first_tk_pos,
            end_tk: self.pos,
        }))
    }

    fn parse_return(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;
        self.consume(); // ? skip 'return'

        if self.tokens[self.pos].kind == TokenKind::Semicolon {
            self.consume();

            return Ok(Box::new(SyntaxNode {
                data: SyntaxData::Return {
                    out: Box::new(SyntaxNode {
                        data: SyntaxData::Nil,
                        first_tk: 0,
                         end_tk: 0
                    })
                },
                first_tk: first_tk_pos,
                end_tk: first_tk_pos + 1
            }));
        }

        let out = self.parse_or()?;

        let semicolon_tk = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr { culprit: semicolon_tk.clone(), msg: "Expected ';' ending this return-stmt.", line: semicolon_tk.line }, semicolon_tk.clone(), TokenKind::Semicolon);

        Ok(Box::new(SyntaxNode {
            data: SyntaxData::Return { out },
            first_tk: first_tk_pos,
            end_tk: self.pos,
        }))
    }

    fn parse_expr_stmt(&mut self) -> Result<Box<SyntaxNode>, ParseErr> {
        let first_tk_pos = self.pos;
        let lhs = self.parse_or()?;

        if self.tokens[self.pos].kind == TokenKind::OperatorAssign {
            self.consume();

            let src = self.parse_or()?;

            let semicolon_tk = &self.tokens[self.pos];
            CONSUME_OF!(self.consume(), ParseErr { culprit: semicolon_tk.clone(), msg: "Expected ';' ending this assignment.", line: semicolon_tk.line }, semicolon_tk.clone(), TokenKind::Semicolon);

            return Ok(Box::new(SyntaxNode {
                data: SyntaxData::ExprStmt {
                    inner: Box::new(SyntaxNode {
                        data: SyntaxData::Assign {
                            dest: lhs, src
                        },
                        first_tk: first_tk_pos,
                        end_tk: self.pos,
                    })
                },
                first_tk: first_tk_pos,
                end_tk: self.pos,
            }));
        }

        let semicolon_tk = &self.tokens[self.pos];
        CONSUME_OF!(self.consume(), ParseErr { culprit: semicolon_tk.clone(), msg: "Expected ';' ending this assignment.", line: semicolon_tk.line }, semicolon_tk.clone(), TokenKind::Semicolon);

        Ok(lhs)
    }

    pub fn parse_data(&mut self) -> Option<Vec<Box<SyntaxNode>>> {
        let mut decls = Vec::<Box<SyntaxNode>>::default();

        while !self.at_eof() {
            let parse_result = self.parse_stmt();

            if parse_result.is_ok() {
                decls.push(parse_result.ok().expect("Expected parsed stmt in Parser::parse_data()!"));
            } else  {
                let parse_error = parse_result.err().expect("Expected parser error in Parser::parse_data()!");
                self.errors += 1;
                eprintln!("Parse error #{} [source:{}]:\n{}\n\n", self.errors, parse_error.line, parse_error.msg);
                self.recover();
            }
        }

        Some(decls)
    }
}
