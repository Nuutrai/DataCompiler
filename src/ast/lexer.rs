use std::cmp;

#[derive(Debug)]
enum TokenKind {
    Data,
    Literal,
    RightParen,
    LeftParen,
    Equals,
    Eof,
    Bad,
    Error,
}

#[derive(Debug)]
pub struct TextSpan {
    start: usize,
    end: usize,
    line: usize,
    literal: String,
}

impl TextSpan {
    pub fn new(start: usize, end: usize, line: usize, literal: String) -> Self {
        Self {
            start,
            end,
            line,
            literal,
        }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }
}

#[derive(Debug)]
pub struct Token {
    kind: TokenKind,
    span: TextSpan,
}

impl Token {
    fn new(kind: TokenKind, span: TextSpan) -> Token {
        Token { kind, span }
    }
}

pub(crate) struct Lexer {
    input: String,
    current_pos: usize,
    current_line: usize,
}

impl Lexer {
    pub fn new(input: String) -> Self {
        Self {
            input,
            current_pos: 0,
            current_line: 1,
        }
    }
    
    pub fn tokenize(mut self) -> Vec<Token> {
        if self.current_pos >= self.input.len() {
            // return vec!(Token::new(TokenKind::Error, TextSpan::new(self.current_pos, self.current_pos, "Input already tokenized".to_string())));
        }

        self.skip_whitespace();
        self.skip_comments();

        let mut vec: Vec<Token> = Vec::new();
        
        while let Some(c) = self.consume() {
            
            if c.is_whitespace() {
                continue;
            }

            if c == '(' {
                vec.push(Token::new(
                    TokenKind::LeftParen,
                    TextSpan::new(self.current_pos, self.current_pos, self.current_line, "(".to_string()),
                ));
                
                continue;
            }

            if c == ')' {
                vec.push(Token::new(
                    TokenKind::RightParen,
                    TextSpan::new(self.current_pos, self.current_pos, self.current_line, ")".to_string()),
                ));
                
                continue;
            }
            
            if c == '=' {
                vec.push(Token::new(
                    TokenKind::Equals,
                    TextSpan::new(self.current_pos, self.current_pos, self.current_line, "=".to_string()),
                ))
            }

            if c == '"' {
                self.string_literal_tokenize(self.current_char() == Some('"') || self.peek_char() == Some('"'))
            }
            
            if c.is_alphabetic() {
                vec.push(self.greedy_tokenize(c));
                continue
            }
            
        }

        vec
    }
    
    fn greedy_tokenize(&mut self, c: char) -> Token {
        let mut buffer = c.to_string();
        let start = self.current_pos;
        while let Some(c) = self.consume() {
            buffer.push(c);

            match self.current_char() {
                Some(peek) if !peek.is_alphanumeric() => {
                    let end = self.current_pos;
                    if buffer.to_lowercase() == "data" {
                        return Token::new(TokenKind::Data, TextSpan::new(start, end, self.current_line, buffer));
                    }
                    return Token::new(TokenKind::Literal, TextSpan::new(start, end, self.current_line, buffer));
                }
                None => {
                    
                    // todo make sure there's no need for #consume
                    
                    let end = self.current_pos;
                    if buffer.to_lowercase() == "data" {
                        return Token::new(TokenKind::Data, TextSpan::new(start, end, self.current_line, buffer));
                    }
                    return Token::new(TokenKind::Literal, TextSpan::new(start, end, self.current_line, buffer));
                }
                Some(_) => (),
            }
        }

        Token::new(
            TokenKind::Error,
            TextSpan::new(
                start,
                self.current_pos,
                self.current_line,
                "Literal does not terminate".to_string(),
            ),
        )
    }
    
    fn string_literal_tokenize(&mut self, is_multi_line: bool) {
        if is_multi_line {
            self.consume();
            self.consume();
        }
        let mut buffer = String::new();
        while let Some(c) = self.consume() {
            buffer.push(c);
            
            if self.current_char() == Some('"') {
                break;
            }
        }
    }
    
    fn skip_whitespace(&mut self) {
        while self.current_char().unwrap().is_whitespace() && self.current_pos < self.input.len() {
            self.consume();
        }
    }
    
    fn skip_comments(&mut self) {
        while let Some(c) = self.current_char()
            && let Some(peek) = self.peek_char()
        {
            if c != '/' || peek != '/' {
                break;
            }
            'exhaust_comment: while let Some(c) = self.consume() {
                if c == '\n' {
                    break 'exhaust_comment;
                }
            }
        }
    }
    
    fn current_char(&self) -> Option<char> {
        self.input.chars().nth(self.current_pos)
    }
    
    fn peek_char(&self) -> Option<char> {
        self.input.chars().nth(self.current_pos + 1)
    }
    
    fn peek_char_by(&self, by: usize) -> Option<char> {
        self.input.chars().nth(self.current_pos + cmp::max(by, 1))
    }
    
    fn consume(&mut self) -> Option<char> {
        if self.current_pos >= self.input.len() {
            return None;
        }
        if self.peek_char() == Some('\n') {
            self.current_pos += 1;
            self.current_line += 1;
        }
        let c = self.current_char();
        self.current_pos += 1;
        c
    }
    
    // pub fn next_token(&mut self) -> Option<Token> {
    //     self.skip_whitespace();
    //
    //     if self.current_pos == self.input.len() {
    //         return Some(Token::new(
    //             Eof,
    //             TextSpan::new(self.current_pos, self.current_pos, String::new()),
    //         ));
    //     }
    //
    //     let start = self.current_pos;
    //     let c = self.current_char();
    //     c.map(|c| {
    //         let start = self.current_pos;
    //         let mut kind = TokenKind::Bad;
    //         if c.is_digit(10) {
    //             let number = self.consume_number();
    //             kind = TokenKind::Number(number)
    //         }
    //         let end = self.current_pos;
    //         let literal = self.input[start..end].to_string();
    //         let span = TextSpan::new(start, end, literal);
    //         Token::new(kind, span)
    //     })
    // }
    //
    // fn is_number_start(c: &char) -> bool {
    //     c.is_digit(10)
    // }
    //
    // fn consume_number(&mut self) -> i64 {
    //     let mut number: i64 = 0;
    //
    //     'greedy: loop {
    //         match self.current_char() {
    //             None => break 'greedy,
    //             Some(_) => {
    //                 if !self.current_char().unwrap().is_digit(10) {
    //                     println!("NOOOO: {:?}", self.current_char());
    //                     break 'greedy;
    //                 }
    //             }
    //         }
    //
    //         let digit = self.consume().unwrap().to_digit(10);
    //
    //         println!("{:?}", digit);
    //
    //         number = number * 10 + digit.unwrap() as i64;
    //     }
    //
    //     number
    // }
    
}
