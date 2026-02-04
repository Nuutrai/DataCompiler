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
        let mut tokens = Vec::new();
        
        loop {
            self.skip_whitespace();
            
            let Some(c) = self.current_char() else {
                break; // end of input
            };
            
            let token = match c {
                '(' => self.consume_single_char(TokenKind::LeftParen),
                ')' => self.consume_single_char(TokenKind::RightParen),
                '=' => self.consume_single_char(TokenKind::Equals),
                '"' => self.consume_any_string(),
                '/' if self.peek_char() == Some('/') => {
                    self.skip_comments();
                    continue;
                }
                c if c.is_alphabetic() => self.consume_identifier(),
                c if c.is_whitespace() => {
                    self.consume();
                    continue;
                }
                _ => self.consume_error(),
            };
            
            tokens.push(token);
        }
        
        tokens
    }

    fn skip_whitespace(&mut self) {
        self.consume_while(|c| c.is_whitespace());
    }
    
    fn skip_comments(&mut self) {
        while self.current_char() == Some('/') && self.peek_char() == Some('/') {
            self.consume_until(|c| c == '\n');
            if self.current_char() == Some('\n') {
                self.consume();
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
        self.input.chars().nth(self.current_pos + by)
    }
    
    fn consume(&mut self) -> Option<char> {
        let c = self.current_char()?;
        
        if c == '\n' {
            self.current_line += 1;
        }
        
        self.current_pos += 1;
        Some(c)
    }

    fn consume_while<F>(&mut self, predicate: F) -> String 
    where F: Fn(char) -> bool 
    {
        let mut buffer = String::new();
        while let Some(c) = self.current_char() && predicate(c) {
            buffer.push(c);
            self.consume();
        }
        buffer
    }
    
    fn consume_until<F>(&mut self, predicate: F) -> String 
    where F: Fn(char) -> bool 
    {
        self.consume_while(|c| !predicate(c))
    }

    fn consume_n_chars(&mut self, n: usize) {
        (0..n).for_each(|_| { self.consume(); });
    }

    fn consume_single_char(&mut self, kind: TokenKind) -> Token {
        let start = self.current_pos;
        let ch = self.consume().unwrap();
        Token::new(
            kind,
            TextSpan::new(start, self.current_pos, self.current_line, ch.to_string())
        )
    }

    fn consume_identifier(&mut self) -> Token {
        let start = self.current_pos;
        let buffer = self.consume_while(|c| c.is_alphanumeric() || c == '_');
        
        let kind = match buffer.as_str() {
            "data" => TokenKind::Data,
            _ => TokenKind::Literal,
        };
        
        Token::new(kind, TextSpan::new(start, self.current_pos, self.current_line, buffer))
    }

    fn consume_any_string(&mut self) -> Token {
        let start = self.current_pos;
        let quotes = self.count_consecutive_chars('"', 3);

        self.consume_n_chars(quotes); 
        
        let mut buffer = String::new();
        
        while let Some(c) = self.current_char() {
            let at_closing = (0..quotes).all(|i| self.peek_char_by(i) == Some('"'));
            
            match (at_closing, quotes == 1 && c == '\n') {
                (true, _) => break,
                (_, true) => return Token::new(
                    TokenKind::Error,
                    TextSpan::new(start, self.current_pos, self.current_line, 
                        "Unterminated string".to_string())
                ),
                _ => {
                    buffer.push(c);
                    self.consume();
                }
            }
        }
        
        self.consume_n_chars(quotes);
        
        Token::new(
            TokenKind::Literal,
            TextSpan::new(start, self.current_pos, self.current_line, buffer)
        )
    }

    fn consume_error(&mut self) -> Token {
        let start = self.current_pos;
        let ch = self.consume().unwrap_or('\0');
        Token::new(
            TokenKind::Error,
            TextSpan::new(start, self.current_pos, self.current_line, 
                format!("Unexpected character: '{}'", ch))
        )
    }

    fn count_consecutive_chars(&self, ch: char, max: usize) -> usize {
        (0..max)
            .filter(|&i| self.peek_char_by(i) == Some(ch))
            .count()
            .max(1)
    }
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
