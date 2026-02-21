use crate::ast::lexer::TokenKind;
use crate::ast::lexer::Token;

#[derive(Debug, Clone)]
pub enum Expr {
    Number(usize),
    String(String),
    Identifier(String),
    Group(Box<Expr>),

    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },

    Index {
        target: Box<Expr>,
        index: Box<Expr>,
    },

    Ternary {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn current(&self) -> &TokenKind {
        self.tokens
            .get(self.pos)
            .map(|t| &t.kind)
            .unwrap_or(&TokenKind::Eof)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn match_kind(&mut self, kind: &TokenKind) -> bool {
        if std::mem::discriminant(self.current()) == std::mem::discriminant(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn skip_newlines(&mut self) {
        while self.match_kind(&TokenKind::Newline) {}
    }

    pub fn is_at_end(&self) -> bool {
        *self.current() == TokenKind::Eof
    }

    fn parse_primary(&mut self) -> Expr {
        match self.current().clone() {
            TokenKind::Number(n) => {
                self.advance();
                Expr::Number(n)
            }

            TokenKind::StringLiteral(s) => {
                self.advance();
                Expr::String(s)
            }

            TokenKind::Identifier(s) => {
                self.advance();
                Expr::Identifier(s)
            }

            TokenKind::LeftParen => {
                self.advance();
                let expr = self.parse_expr();
                self.match_kind(&TokenKind::RightParen);
                Expr::Group(Box::new(expr))
            }

            other => panic!("Unexpected token: {:?}", other),
        }
    }

    pub fn parse_expr(&mut self) -> Expr {
        let mut expr = self.parse_primary();

        loop {
            if self.match_kind(&TokenKind::LeftParen) {
                let mut args = Vec::new();
                if !self.match_kind(&TokenKind::RightParen) {
                    loop {
                        args.push(self.parse_expr());
                        if self.match_kind(&TokenKind::RightParen) {
                            break;
                        }
                        self.match_kind(&TokenKind::Comma);
                    }
                }
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                };
                continue;
            }

            if self.match_kind(&TokenKind::SquareLeft) {
                let index = self.parse_expr();
                self.match_kind(&TokenKind::SquareRight);
                expr = Expr::Index {
                    target: Box::new(expr),
                    index: Box::new(index),
                };
                continue;
            }

            if self.match_kind(&TokenKind::QuestionMark) {
                let then_branch = self.parse_expr();
                self.match_kind(&TokenKind::Colon);
                let else_branch = self.parse_expr();
                expr = Expr::Ternary {
                    cond: Box::new(expr),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                };
                continue;
            }

            break;
        }

        expr
    }
}
