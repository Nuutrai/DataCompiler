use crate::ast::lexer::{Token, TokenKind};

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

#[derive(Debug, Clone)]
pub enum Stmt {
    Function {
        name: String,
        args: Vec<String>,
        body: Vec<Stmt>,
    },
    AsmFunction {
        arch: String,
        name: String,
        args: Vec<String>,
        body: Vec<Stmt>,
    },
    Variable {
        name: String,
        value: Expr,
    },
    Expr(Expr),
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

    fn peek_by(&self, by: usize) -> &TokenKind {
        self.tokens
            .get(self.pos + by)
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

    fn expect(&mut self, kind: &TokenKind) {
        if !self.match_kind(kind) {
            panic!("Expected {:?} but got {:?}", kind, self.current());
        }
    }

    fn skip_newlines(&mut self) {
        while self.match_kind(&TokenKind::Newline) {}
    }

    pub fn is_at_end(&self) -> bool {
        matches!(self.current(), TokenKind::Eof)
    }

    pub fn parse(&mut self) -> Vec<Stmt> {
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.is_at_end() {
                break;
            }
            stmts.push(self.parse_stmt());
        }
        stmts
    }

    fn parse_stmt(&mut self) -> Stmt {
        match self.current().clone() {
            TokenKind::Data => {
                match self.peek_by(2) {
                    TokenKind::LeftParen => self.parse_function(),
                    TokenKind::Equals => self.parse_variable(),
                    _ => self.parse_asm_function(),
                }
            }
            _ => {
                let expr = self.parse_expr();
                Stmt::Expr(expr)
            }
        }
    }

    fn parse_function(&mut self) -> Stmt {
        self.advance();
        let name = self.expect_identifier();
        self.expect(&TokenKind::LeftParen);
        let args = self.parse_param_list();
        let body = self.parse_body();
        Stmt::Function { name, args, body }
    }

    fn parse_asm_function(&mut self) -> Stmt {
        self.advance();
        let mut arch = String::new();
        while !matches!(self.current(), TokenKind::Dollar | TokenKind::Eof) {
            arch.push_str(&self.current_literal());
            self.advance();
        }
        self.match_kind(&TokenKind::Dollar);
        let name = self.expect_identifier();
        self.expect(&TokenKind::LeftParen);
        let args = self.parse_param_list();
        let body = self.parse_body();
        Stmt::AsmFunction { arch, name, args, body }
    }

    fn parse_variable(&mut self) -> Stmt {
        self.advance();
        let name = self.expect_identifier();
        self.expect(&TokenKind::Equals);
        let value = self.parse_expr();
        Stmt::Variable { name, value }
    }

    fn parse_param_list(&mut self) -> Vec<String> {
        let mut params = Vec::new();
        if !self.match_kind(&TokenKind::RightParen) {
            loop {
                params.push(self.expect_identifier());
                if self.match_kind(&TokenKind::RightParen) {
                    break;
                }
                self.expect(&TokenKind::Comma);
            }
        }
        params
    }

    fn parse_body(&mut self) -> Vec<Stmt> {
        self.expect(&TokenKind::CurlyLeft);
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.match_kind(&TokenKind::CurlyRight) || self.is_at_end() {
                break;
            }
            stmts.push(self.parse_stmt());
        }
        stmts
    }

    fn expect_identifier(&mut self) -> String {
        match self.current().clone() {
            TokenKind::Identifier(s) => {
                self.advance();
                s
            }
            other => panic!("Expected identifier, got {:?}", other),
        }
    }

    fn current_literal(&self) -> String {
        self.tokens
            .get(self.pos)
            .map(|t| t.span.literal.clone())
            .unwrap_or_default()
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
                expr = Expr::Call { callee: Box::new(expr), args };
                continue;
            }
            if self.match_kind(&TokenKind::SquareLeft) {
                let index = self.parse_expr();
                self.match_kind(&TokenKind::SquareRight);
                expr = Expr::Index { target: Box::new(expr), index: Box::new(index) };
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

    fn parse_primary(&mut self) -> Expr {
        match self.current().clone() {
            TokenKind::Number(n) => { self.advance(); Expr::Number(n) }
            TokenKind::StringLiteral(s) => { self.advance(); Expr::String(s) }
            TokenKind::Identifier(s) => { self.advance(); Expr::Identifier(s) }
            TokenKind::LeftParen => {
                self.advance();
                let expr = self.parse_expr();
                self.match_kind(&TokenKind::RightParen);
                Expr::Group(Box::new(expr))
            }
            other => panic!("Unexpected token in expression: {:?}", other),
        }
    }
}
