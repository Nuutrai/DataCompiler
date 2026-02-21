use crate::ast::lexer::{Token, TokenKind};

pub(crate) struct Parser {
    tokens: Vec<Token>,
    output: String,
    current_pos: usize,
}

// pub(crate) trait Parser {
//     fn new(tokens: Vec<Token>) -> Self;
//
//     fn parse(mut self);
//
//     // fn parse_function(&mut self);
//
//     fn current_token(&self) -> Option<&Token>;
//
//     // fn peek_token(&self) -> Option<&Token>;
//
//     fn peek_token_by(&self, by: usize) -> Option<&Token>;
//
//     fn consume(&mut self) -> Option<&Token>;
//
//     fn consume_n_tokens(&mut self, n: usize);
// }

impl Parser {
    pub(crate) fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens: tokens,
            output: String::new(),
            current_pos: 0
        }
    }

    pub(crate) fn parse(mut self) {
        loop {
            let Some(t) = self.current_token() else {
                break;
            };

            match &t.kind {
                TokenKind::Data => {
                    let Some(p2) = self.peek_token_by(2) else {
                        break;
                    };
                    match p2.kind {
                        TokenKind::LeftParen => {
                            self.parse_function();
                        }
                        TokenKind::Equals => {
                            self.parse_variable_declaration();
                        }
                        _ => {
                            self.parse_asm_function()
                        }
                    }
                }
                _ => {
                    self.consume();
                },
            }
        }
    }

    fn parse_asm_function(&mut self) {
        self.consume();
        let mut arch = String::new();
        while let Some(token) = self.current_token() {
            match token.kind {
                TokenKind::Dollar => {
                    self.consume();
                    break;
                }
                _ => {}
            }

            arch.push_str(&(token.span.literal.as_str()));

            self.consume();
        }

        // let mut function = String::new();
        //
        // let Some(literal) = self.current_token() else {
        //     panic!("Whoa whoa whoa you messed up");
        // };
        //
        // match &literal.kind {
        //     TokenKind::Literal(literal) => {
        //         function.push_str(literal.as_str());
        //     }
        //     _ => {}
        // }


        self.parse_function();

        println!("ASM function for {} named ...", arch) // function);
    }

    fn parse_function(&mut self) {
        self.consume();
        let mut name = String::new();
        let Some(token) = self.current_token() else {
            panic!("Declared a function, but it's name is at the end of the file?");
        };

        match &token.kind {
            TokenKind::Literal(literal) => {
                name.push_str(literal.as_str());
            }
            _ => {
                panic!("Function names must be alphanumeric (including _) and start with a letter from the alphabet");
            }
        }

        self.consume();

        // Argument time
        if self.current_token().is_none() {
            panic!("Incorrect function layout")
        }

        self.consume();

        let mut arguments: Vec<&str> = Vec::new();

        loop {
            let Some(token) = self.current_token() else {
                panic!("Incomplete function declaration");
            };

        }

        println!("Function {}", name);
    }

    fn parse_variable_declaration(&mut self) {
        self.consume();
        let mut name = String::new();
        let Some(token) = self.consume() else {
            panic!("Declared a variable, but it's name is at the end of the file?");
        };
        match &token.kind {
            TokenKind::Literal(literal) => {
                name.push_str(literal.as_str());
            }
            _ => {}
        }

        self.consume();

        println!("Variable {}", name);
    }

    fn current_token(&self) -> Option<&Token> {
        if self.current_pos >= self.tokens.len() {
            return None;
        }
        Some(&self.tokens[self.current_pos])
    }

    fn peek_token(&self) -> Option<&Token> {
        if self.current_pos + 1 >= self.tokens.len() {
            return None;
        }
        Some(&self.tokens[self.current_pos + 1])
    }

    fn peek_token_by(&self, by: usize) -> Option<&Token> {
        if self.current_pos + by >= self.tokens.len() {
            return None;
        }
        Some(&self.tokens[self.current_pos + by])
    }

    fn consume(&mut self) -> Option<&Token> {
        if self.current_pos >= self.tokens.len() {
            return None;
        }
        let token = &self.tokens[self.current_pos];

        self.current_pos += 1;
        Some(token)
    }

    fn consume_n_tokens(&mut self, n: usize) {
        (0..n).for_each(|_| {
            self.consume();
        });
    }
}
