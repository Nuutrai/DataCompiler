use crate::{
    ast::lexer::{TextSpan, Token, TokenKind},
    error::{ErrorKind, ErrorReporter},
};

#[derive(Debug, Clone)]
pub enum Type {
    Data(u8),
    DataArray(u8, usize),
    Named(String),
    Ref(Box<Type>),
    Pointer(Box<Type>),
    Generic(String, Vec<Type>),
    Void,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(usize, TextSpan),
    String(String, TextSpan),
    Identifier(String, TextSpan),
    Pointer(Box<Expr>, TextSpan),
    Group(Box<Expr>, TextSpan),
    List(Vec<Expr>, TextSpan),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: TextSpan,
    },
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
        span: TextSpan,
    },
    Ternary {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        span: TextSpan,
    },
    Field {
        target: Box<Expr>,
        field: String,
        span: TextSpan,
    },
}

impl Expr {
    pub fn span(&self) -> &TextSpan {
        match self {
            Expr::Number(_, s) => s,
            Expr::String(_, s) => s,
            Expr::Pointer(_, s) => s,
            Expr::Identifier(_, s) => s,
            Expr::Group(_, s) => s,
            Expr::Field { span, .. } => span,
            Expr::Call { span, .. } => span,
            Expr::Index { span, .. } => span,
            Expr::Ternary { span, .. } => span,
            Expr::List(_, s) => s,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Statement {
    Function {
        name: String,
        args: Vec<(String, Type)>,
        returns: Option<Type>,
        body: Vec<Statement>,
        span: TextSpan,
    },
    AsmFunction {
        arch: String,
        name: String,
        args: Vec<Type>,
        returns: Option<Type>,
        body: String,
        span: TextSpan,
    },
    Variable {
        name: String,
        ty: Option<Type>,
        value: Option<Expr>,
        span: TextSpan,
    },
    Assign {
        target: AssignTarget,
        value: Expr,
        span: TextSpan,
    },
    Struct {
        name: String,
        generics: Vec<String>,
        fields: Vec<(String, Type)>,
        span: TextSpan,
    },
    Expr(Expr),
    Import(String, TextSpan),
}

#[derive(Debug, Clone)]
pub enum AssignTarget {
    Variable(String),
    Index(String, Box<Expr>),
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    file: String,
    pub errors: ErrorReporter,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, file: &str) -> Self {
        Self {
            tokens,
            pos: 0,
            file: file.to_string(),
            errors: ErrorReporter::new(file),
        }
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
            let span = self.span();
            self.errors.error(
                ErrorKind::UnexpectedToken {
                    expected: format!("{:?}", kind),
                    got: format!("{:?}", self.current()),
                },
                span.line,
                span.start,
                span.len(),
            );
        }
    }

    fn skip_newlines(&mut self) {
        while self.match_kind(&TokenKind::Newline) {}
    }

    pub fn is_at_end(&self) -> bool {
        matches!(self.current(), TokenKind::Eof) || self.pos == self.tokens.len() - 1
    }

    fn expect_identifier(&mut self) -> String {
        match self.current().clone() {
            TokenKind::Identifier(s) => {
                self.advance();
                s
            }
            other => {
                let span = self.span();
                self.errors.error(
                    ErrorKind::UnexpectedToken {
                        expected: "identifier".to_string(),
                        got: format!("{:?}", other),
                    },
                    span.line,
                    span.start,
                    span.len(),
                );
                String::new()
            }
        }
    }

    fn current_literal(&self) -> String {
        self.tokens
            .get(self.pos)
            .map(|t| t.span.literal.clone())
            .unwrap_or_default()
    }

    fn span(&self) -> TextSpan {
        self.tokens
            .get(self.pos)
            .unwrap_or(&Token::new(TokenKind::Eof, TextSpan::new(0, 0, 0, String::from("Error: Invaid token"))))
            .span.clone()
    }

    pub fn parse(&mut self) -> Vec<Statement> {
        let mut statements = Vec::new();
        loop {
            self.skip_newlines();
            if self.is_at_end() {
                break;
            }
            let span = self.span();
            let literal = self.current_literal();
            let stmt = self.parse_statement();
            match &stmt {
                Statement::Function { .. }
                | Statement::AsmFunction { .. }
                | Statement::Import { .. }
                | Statement::Variable { .. }
                | Statement::Struct { .. } => {}
                _ => {
                    self.errors.error(
                        ErrorKind::UnexpectedToken {
                            expected: "function, variable, or import".to_string(),
                            got: literal.clone(),
                        },
                        span.line,
                        span.start,
                        literal.len(),
                    );
                }
            }
            statements.push(stmt);
        }
        statements
    }

    fn parse_statement(&mut self) -> Statement {
        match self.current().clone() {
            TokenKind::Data(_) => self.parse_data(),
            TokenKind::Identifier(name) => match self.peek_by(1) {
                TokenKind::Equals => self.parse_assign(),
                TokenKind::SquareLeft => self.parse_index_assign(),
                _ => Statement::Expr(self.parse_expr()),
            },
            _ => Statement::Expr(self.parse_expr()),
        }
    }

    fn parse_assign(&mut self) -> Statement {
        let span = self.span();
        let name = self.expect_identifier();
        self.expect(&TokenKind::Equals);
        let value = self.parse_expr();
        Statement::Assign {
            target: AssignTarget::Variable(name),
            value,
            span,
        }
    }

    fn parse_index_assign(&mut self) -> Statement {
        let span = self.span();
        let name = self.expect_identifier();
        self.expect(&TokenKind::SquareLeft);
        let index = self.parse_expr();
        self.expect(&TokenKind::SquareRight);
        self.expect(&TokenKind::Equals);
        let value = self.parse_expr();
        Statement::Assign {
            target: AssignTarget::Index(name, Box::new(index)),
            value,
            span,
        }
    }

    fn parse_data(&mut self) -> Statement {
        self.advance();
        match self.current() {
            TokenKind::LeftParen => {
                let span = self.span();
                self.advance();
                let path = match self.current().clone() {
                    TokenKind::StringLiteral(s) => {
                        self.advance();
                        s
                    }
                    other => {
                        let span = self.span();
                        self.errors.error(
                            ErrorKind::UnexpectedToken {
                                expected: "import path".to_string(),
                                got: format!("{:?}", other),
                            },
                            span.line,
                            span.start,
                            span.len(),
                        );
                        String::new()
                    }
                };
                self.expect(&TokenKind::RightParen);
                Statement::Import(path, span)
            }
            _ => match self.peek_by(1) {
                TokenKind::LeftParen => self.parse_function(),
                TokenKind::Equals => self.parse_variable(),
                TokenKind::Colon => self.parse_variable(),
                TokenKind::AngledLeft => self.parse_struct(), // data Foo<T> {
                TokenKind::CurlyLeft => self.parse_struct(),  // data Foo {
                _ => self.parse_asm_function(),
            },
        }
    }

    fn parse_function(&mut self) -> Statement {
        let span = self.span();
        let name = self.expect_identifier();
        self.expect(&TokenKind::LeftParen);
        let args = self.parse_param_list();

        let returns = if !self.match_kind(&TokenKind::Colon) {
            None
        } else {
            Some(self.parse_type())
        };
        
        let body = self.parse_body();
        Statement::Function {
            name,
            args,
            returns,
            body,
            span,
        }
    }

    fn parse_variable(&mut self) -> Statement {
        let span = self.span();
        let name = self.expect_identifier();
        let ty = if self.match_kind(&TokenKind::Colon) {
            Some(self.parse_type())
        } else {
            None
        };
        let value = if self.match_kind(&TokenKind::Equals) {
            Some(self.parse_expr())
        } else {
            None
        };
        Statement::Variable {
            name,
            ty,
            value,
            span,
        }
    }

    fn parse_asm_function(&mut self) -> Statement {
        let span = self.span();
        let mut arch = String::new();
        while !matches!(self.current(), TokenKind::Dollar | TokenKind::Eof) {
            arch.push_str(&self.current_literal());
            self.advance();
        }
        self.expect(&TokenKind::Dollar);
        let name = self.expect_identifier();
        self.expect(&TokenKind::LeftParen);
        let args = self.parse_asm_param_list();
        
        let returns = if !self.match_kind(&TokenKind::Colon) {
            None
        } else {
            Some(self.parse_type())
        };
        
        let body = self.parse_asm_body();
        Statement::AsmFunction {
            arch,
            name,
            args,
            returns,
            body,
            span,
        }
    }

    fn parse_type(&mut self) -> Type {
        // if self.match_kind(&TokenKind::Ampersand) {
        //     return Type::Ref(Box::new(self.parse_type()));
        // }

        match self.current().clone() {
            TokenKind::Data(bits) => {
                self.advance();
                if self.match_kind(&TokenKind::SquareLeft) {
                    let size = match self.current().clone() {
                        TokenKind::Number(n) => {
                            self.advance();
                            n
                        }
                        _ => 0,
                    };
                    self.expect(&TokenKind::SquareRight);
                    Type::DataArray(bits.unwrap_or(8), size)
                } else if self.match_kind(&TokenKind::Ampersand) {
                    Type::Pointer(Box::from(Type::Data(bits.unwrap_or(8))))
                } else {
                    if bits == Some(0) {
                        Type::Void
                    } else {
                        Type::Data(bits.unwrap_or(8))
                    }
                }
            }
            TokenKind::Identifier(_) => {
                let name = self.expect_identifier();
                if self.match_kind(&TokenKind::AngledLeft) {
                    let mut params = Vec::new();
                    if !self.match_kind(&TokenKind::AngledRight) {
                        loop {
                            params.push(self.parse_type());
                            if self.match_kind(&TokenKind::AngledRight) {
                                break;
                            }
                            self.expect(&TokenKind::Comma);
                        }
                    }
                    Type::Generic(name, params)
                } else {
                    Type::Named(name)
                }
            }
            other => {
                let span = self.span();
                self.errors.error(
                    ErrorKind::UnexpectedToken {
                        expected: "type".to_string(),
                        got: format!("{:?}", other),
                    },
                    span.line,
                    span.start,
                    span.len(),
                );
                Type::Data(8)
            }
        }
    }

    fn parse_struct(&mut self) -> Statement {
        let span = self.span();
        let name = self.expect_identifier();

        let generics = if self.match_kind(&TokenKind::AngledLeft) {
            let mut params = Vec::new();
            if !self.match_kind(&TokenKind::AngledRight) {
                loop {
                    params.push(self.expect_identifier());
                    if self.match_kind(&TokenKind::AngledRight) {
                        break;
                    }
                    self.expect(&TokenKind::Comma);
                }
            }
            params
        } else {
            Vec::new()
        };

        self.expect(&TokenKind::CurlyLeft);
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if self.match_kind(&TokenKind::CurlyRight) || self.is_at_end() {
                break;
            }
            let fname = self.expect_identifier();
            self.expect(&TokenKind::Colon);
            let ftype = self.parse_type();
            fields.push((fname, ftype));
            self.skip_newlines();
        }

        Statement::Struct {
            name,
            generics,
            fields,
            span,
        }
    }

    // TODO Look into making the two param list functions into one with an asm argument :p
    fn parse_param_list(&mut self) -> Vec<(String, Type)> {
        let mut params = Vec::new();
        if !self.match_kind(&TokenKind::RightParen) {
            loop {
                let name = self.expect_identifier();
                self.expect(&TokenKind::Colon);
                let ty = self.parse_type();
                params.push((name, ty));
                if self.match_kind(&TokenKind::RightParen) {
                    break;
                }
                self.expect(&TokenKind::Comma);
            }
        }
        params
    }

    fn parse_asm_param_list(&mut self) -> Vec<Type> {
        let mut params = Vec::new();
        if !self.match_kind(&TokenKind::RightParen) {
            loop {
                let ty = self.parse_type();
                params.push(ty);
                if self.match_kind(&TokenKind::RightParen) {
                    break;
                }
                self.expect(&TokenKind::Comma);
            }
        }
        params
    }

    fn parse_body(&mut self) -> Vec<Statement> {
        self.expect(&TokenKind::CurlyLeft);
        let mut statements = Vec::new();
        loop {
            self.skip_newlines();
            if self.match_kind(&TokenKind::CurlyRight) || self.is_at_end() {
                break;
            }
            statements.push(self.parse_statement());
            self.skip_newlines();
        }
        statements
    }

    fn parse_asm_body(&mut self) -> String {
        self.expect(&TokenKind::CurlyLeft);
        let mut body = String::new();
        loop {
            match self.current() {
                TokenKind::CurlyRight | TokenKind::Eof => {
                    self.advance();
                    break;
                }
                TokenKind::Newline => {
                    body.push('\n');
                    self.advance();
                }
                TokenKind::Comma => {
                    body.push_str(", ");
                    self.advance();
                }
                _ => {
                    let lit = self.current_literal();
                    body.push_str(&lit);
                    let next = self.tokens.get(self.pos + 1).map(|t| &t.kind);
                    if !matches!(self.current(), TokenKind::Period)
                        && !matches!(
                            next,
                            Some(TokenKind::Newline) | Some(TokenKind::Comma) | None
                        )
                    {
                        body.push(' ');
                    }
                    self.advance();
                }
            }
        }
        body.trim().to_string()
    }

    pub fn parse_expr(&mut self) -> Expr {
        let mut expr = self.parse_primary();
        loop {
            let span = self.span();
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
                    span,
                };
                continue;
            }
            if self.match_kind(&TokenKind::SquareLeft) {
                let index = self.parse_expr();
                self.match_kind(&TokenKind::SquareRight);
                expr = Expr::Index {
                    target: Box::new(expr),
                    index: Box::new(index),
                    span,
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
                    span,
                };
                continue;
            }
            break;
        }
        expr
    }

    fn parse_primary(&mut self) -> Expr {
        let span = self.span();
        match self.current().clone() {
            TokenKind::Number(n) => {
                self.advance();
                Expr::Number(n, span)
            }
            TokenKind::StringLiteral(s) => {
                self.advance();
                Expr::String(s, span)
            }
            TokenKind::Char(b) => {
                self.advance();
                Expr::Number(b as usize, span)
            }
            TokenKind::SquareLeft => {
                self.advance();
                let mut values = Vec::new();
                if self.match_kind(&TokenKind::SquareRight) {
                    // TODO Warnings
                    return Expr::List(values, span);
                };
                loop {
                    values.push(self.parse_expr());
                    if self.match_kind(&TokenKind::SquareRight) {
                        break;
                    }
                    self.match_kind(&TokenKind::Comma);
                }
                Expr::List(values, span)
            }
            TokenKind::Ampersand => {
                self.advance();
                let expr = self.parse_expr();
                Expr::Pointer(Box::new(expr), span)
            }
            TokenKind::Identifier(s) => {
                self.advance();
                Expr::Identifier(s, span)
            }
            TokenKind::LeftParen => {
                self.advance();
                let expr = self.parse_expr();
                self.match_kind(&TokenKind::RightParen);
                Expr::Group(Box::new(expr), span)
            }
            TokenKind::Eof | TokenKind::CurlyRight => Expr::Number(0, span),
            other => {
                self.errors.error(
                    ErrorKind::UnexpectedToken {
                        expected: "expression".to_string(),
                        got: format!("{:?}", other),
                    },
                    span.line,
                    span.start,
                    span.len(),
                );
                self.advance();
                Expr::Number(0, span)
            }
        }
    }
}
