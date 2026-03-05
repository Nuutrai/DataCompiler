#[derive(Debug, Clone)]
pub enum TokenKind {
    Data,
    StringLiteral(String),
    Identifier(String),
    Number(usize),
    Char(u8),

    RightParen,
    LeftParen,
    CurlyLeft,
    CurlyRight,
    AngledLeft,
    AngledRight,
    SquareLeft,
    SquareRight,

    QuestionMark,
    Colon,
    Equals,
    Comma,
    Exclamation,
    Ampersand,
    Plus,
    Minus,
    Asterisk,
    Slash,
    Period,
    Percent,
    Caret,
    Pipe,
    Dollar,

    Newline,
    Eof,
    Bad,
    Error,
}

#[derive(Debug, Clone)]
pub struct TextSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) line: usize,
    pub(crate) literal: String,
}

impl TextSpan {
    pub fn new(start: usize, end: usize, line: usize, literal: String) -> Self {
        Self {
            start: start + 1,
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
    pub(crate) kind: TokenKind,
    pub(crate) span: TextSpan,
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
            input: input.replace("\r\n", "\n"),
            current_pos: 0,
            current_line: 1,
        }
    }

    pub fn tokenize(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace();

            let Some(c) = self.current_char() else {
                break;
            };

            let token = match c {
                '\n' => self.consume_single_char(TokenKind::Newline),
                '/' if self.peek_char() == Some('/') => {
                    self.skip_comments();
                    continue;
                }

                '(' => self.consume_single_char(TokenKind::LeftParen),
                ')' => self.consume_single_char(TokenKind::RightParen),
                '{' => self.consume_single_char(TokenKind::CurlyLeft),
                '}' => self.consume_single_char(TokenKind::CurlyRight),
                '[' => self.consume_single_char(TokenKind::SquareLeft),
                ']' => self.consume_single_char(TokenKind::SquareRight),
                '<' => self.consume_single_char(TokenKind::AngledLeft),
                '>' => self.consume_single_char(TokenKind::AngledRight),
                '?' => self.consume_single_char(TokenKind::QuestionMark),
                ':' => self.consume_single_char(TokenKind::Colon),
                '=' => self.consume_single_char(TokenKind::Equals),
                ',' => self.consume_single_char(TokenKind::Comma),
                '!' => self.consume_single_char(TokenKind::Exclamation),
                '&' => self.consume_single_char(TokenKind::Ampersand),
                '+' => self.consume_single_char(TokenKind::Plus),
                '-' => self.consume_single_char(TokenKind::Minus),
                '*' => self.consume_single_char(TokenKind::Asterisk),
                '/' => self.consume_single_char(TokenKind::Slash),
                '.' => self.consume_single_char(TokenKind::Period),
                '%' => self.consume_single_char(TokenKind::Percent),
                '^' => self.consume_single_char(TokenKind::Caret),
                '|' => self.consume_single_char(TokenKind::Pipe),
                '$' => self.consume_single_char(TokenKind::Dollar),

                '\'' => self.consume_char_literal(),
                '"' => self.consume_any_string(),
                c if c.is_alphabetic() => self.consume_identifier(),
                c if c.is_digit(10) => self.consume_number(),
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
        self.consume_while(None, |c| c == ' ' || c == '\t' || c == '\r');
    }

    fn skip_comments(&mut self) {
        while self.current_char() == Some('/') && self.peek_char() == Some('/') {
            self.consume_until(None, |c| c == '\n');
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

    fn consume_while<F>(&mut self, mut buffer: Option<&mut String>, predicate: F)
    where
        F: Fn(char) -> bool,
    {
        while let Some(c) = self.current_char()
            && predicate(c)
        {
            self.consume();

            if let Some(ref mut buf) = buffer {
                buf.push(c);
            } else {
                continue;
            };
        }
    }

    fn consume_until<F>(&mut self, buffer: Option<&mut String>, predicate: F)
    where
        F: Fn(char) -> bool,
    {
        self.consume_while(buffer, |c| !predicate(c))
    }

    fn consume_n_chars(&mut self, n: usize) {
        (0..n).for_each(|_| {
            self.consume();
        });
    }

    fn consume_single_char(&mut self, kind: TokenKind) -> Token {
        let start = self.current_pos;
        let ch = self.consume().unwrap();
        Token::new(
            kind,
            TextSpan::new(start, self.current_pos, self.current_line, ch.to_string()),
        )
    }

    fn consume_escape(&mut self) -> Option<u8> {
        self.consume(); // consume the backslash
        match self.current_char() {
            Some('n') => {
                self.consume();
                Some(b'\n')
            }
            Some('t') => {
                self.consume();
                Some(b'\t')
            }
            Some('r') => {
                self.consume();
                Some(b'\r')
            }
            Some('0') => {
                self.consume();
                Some(b'\0')
            }
            Some('\\') => {
                self.consume();
                Some(b'\\')
            }
            Some('\'') => {
                self.consume();
                Some(b'\'')
            }
            _ => None,
        }
    }

    fn consume_char_literal(&mut self) -> Token {
        let start = self.current_pos;
        self.consume(); // opening '

        let ch = match self.current_char() {
            Some('\\') => match self.consume_escape() {
                Some(b) => b,
                None => return self.error_token(start, "Unknown escape sequence"),
            },
            Some(c) if c != '\'' => {
                let b = c as u8;
                self.consume();
                b
            }
            _ => return self.error_token(start, "Empty character literal"),
        };

        if self.current_char() != Some('\'') {
            return self.error_token(start, "Unterminated character literal");
        }
        self.consume(); // closing '
        Token::new(
            TokenKind::Char(ch),
            TextSpan::new(
                start,
                self.current_pos,
                self.current_line,
                (ch as char).to_string(),
            ),
        )
    }

    fn consume_identifier(&mut self) -> Token {
        let start = self.current_pos;
        let mut buffer = String::new();
        self.consume_while(Some(&mut buffer), |c| c.is_alphanumeric() || c == '_');

        let kind = match buffer.as_str() {
            "data" => TokenKind::Data,
            _ => TokenKind::Identifier(buffer.clone()),
        };

        Token::new(
            kind,
            TextSpan::new(start, self.current_pos, self.current_line, buffer),
        )
    }

    fn consume_number(&mut self) -> Token {
        let start = self.current_pos;
        let mut buffer = String::new();
        while let Some(_) = self.current_char() {
            self.consume_while(Some(&mut buffer), |c| c.is_digit(10));

            if self.current_char() != Some('_') {
                break;
            }

            self.consume();
        }

        let mut number: usize = 0;

        for c in buffer.chars() {
            number *= 10;
            number += c.to_digit(10).unwrap() as usize;
        }

        Token::new(
            TokenKind::Number(number),
            TextSpan::new(start, self.current_pos, self.current_line, buffer),
        )
    }

    fn consume_any_string(&mut self) -> Token {
        let start = self.current_pos;
        let quotes = self.count_consecutive_chars('"', 3);

        self.consume_n_chars(quotes);

        let mut buffer = String::new();

        while let Some(c) = self.current_char() {
            let at_closing = (0..quotes).all(|i| self.peek_char_by(i) == Some('"'));

            match (at_closing, quotes == 1 && (c == '\n' || c == '\r')) {
                (true, _) => break,
                (_, true) => {
                    return Token::new(
                        TokenKind::Error,
                        TextSpan::new(
                            start,
                            self.current_pos,
                            self.current_line,
                            "Unterminated string".to_string(),
                        ),
                    );
                }
                _ => {
                    buffer.push(c);
                    self.consume();
                }
            }
        }

        self.consume_n_chars(quotes);

        Token::new(
            TokenKind::StringLiteral(buffer.clone()),
            TextSpan::new(start, self.current_pos, self.current_line, buffer),
        )
    }

    fn consume_error(&mut self) -> Token {
        let start = self.current_pos;
        let ch = self.consume().unwrap_or('\0');
        self.error_token(start, &format!("Unexpected character: '{}'", ch))
    }

    fn error_token(&mut self, start: usize, msg: &str) -> Token {
        Token::new(
            TokenKind::Error,
            TextSpan::new(start, self.current_pos, self.current_line, msg.to_string()),
        )
    }

    fn count_consecutive_chars(&self, ch: char, max: usize) -> usize {
        (0..max)
            .take_while(|&i| self.peek_char_by(i) == Some(ch))
            .count()
            .max(1)
    }
}
