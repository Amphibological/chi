//! The Elgin lexer

use std::fmt;

use crate::errors::{Logger, Span};

const SPECIAL_CHARS: [char; 9] = ['(', ')', '[', ']', '{', '}', ',', '=', ':'];

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // literals
    IntLiteral(String),
    FloatLiteral(String),
    StrLiteral(String),

    // identifier
    Ident(String),

    // operator
    Op(String),

    // documentation comment
    DocComment(String),

    // keywords
    Proc,
    If,
    Elif,
    Else,
    While,
    Loop,
    Var,
    Const,
    Return,
    Use,
    Break,
    Continue,

    // special characters
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Equals,
    Colon,

    // newline
    Newline,

    // end of file (used by parser)
    EOF,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct Lexer<'l> {
    code: &'l [char],
    index: usize,
    nesting: usize,
}

impl<'l> Lexer<'l> {
    pub fn new(code: &'l [char]) -> Self {
        Lexer {
            code,
            index: 0,
            nesting: 0,
        }
    }

    fn peek(&self) -> char {
        if self.index >= self.code.len() {
            return '\0';
        }
        self.code[self.index]
    }

    fn next(&mut self) -> char {
        self.index += 1;
        if self.index >= self.code.len() {
            return '\0';
        }
        let ch = self.code[self.index - 1];
        ch
    }

    fn ident_str(&mut self) -> String {
        let mut ident = String::new();
        while is_ident(self.peek()) {
            ident.push(self.next());
        }
        ident
    }

    fn number(&mut self) -> Token {
        let mut number = String::new();
        let mut decimal_passed = false;

        while is_number(self.peek(), decimal_passed) {
            number.push(match self.next() {
                '.' => {
                    decimal_passed = true;
                    '.'
                }
                c => c,
            });
        }
        if decimal_passed {
            Token::FloatLiteral(number)
        } else {
            Token::IntLiteral(number)
        }
    }

    fn operator(&mut self) -> Token {
        let mut op = String::new();
        while is_op(self.peek()) {
            op.push(self.next());
        }
        Token::Op(op)
    }

    fn string(&mut self) -> Option<Token> {
        let mut string = String::new();
        self.next(); // skip "
        while self.peek() != '"' {
            if self.peek() == '\0' {
                Logger::syntax_error("Encountered end of file while parsing string literal", self.index, string.len());
                return None
            }
            string.push(self.next());
        }
        self.next(); // skip "
        Some(Token::StrLiteral(string))
    }

    fn special(&mut self) -> Token {
        match self.peek() {
            '(' | '[' => self.nesting += 1,
            ')' | ']' => self.nesting -= 1,
            ',' | '=' | ':' | '{' | '}' => (),
            _ => unreachable!(),
        };
        match self.next() {
            '(' => Token::LParen,
            ')' => Token::RParen,
            '[' => Token::LBracket,
            ']' => Token::RBracket,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            ',' => Token::Comma,
            '=' => Token::Equals,
            ':' => Token::Colon,
            _ => unreachable!(),
        }
    }

    fn comment(&mut self) {
        self.next(); // throwaway initial #
        while self.peek() != '\n' && self.peek() != '\0' {
            self.next();
        }
        self.next();
    }

    fn doc_comment(&mut self) -> Token {
        self.next(); // throwaway initial #
        self.next(); // throwaway initial :
        let mut doc_comment = String::new();
        while self.peek() != '\n' && self.peek() != '\0' {
            doc_comment.push(self.next());
        }
        self.next();
        Token::DocComment(doc_comment)
    }

    pub fn go(&mut self) -> Option<Vec<Span<Token>>> {
        let mut tokens = vec![];
        loop {
            match self.peek() {
                ch if is_ident_start(ch) => {
                    let id = self.ident_str();
                    tokens.push(
                        self.spanned(str_to_keyword(&id).unwrap_or_else(|| str_to_ident(&id))),
                    );
                }
                '.' => {
                    if is_number(self.code[self.index + 1], false) {
                        let number = self.number();
                        tokens.push(self.spanned(number));
                    } else {
                        tokens.push(self.spanned(Token::Op(".".to_owned())));
                        self.next();
                    }
                }
                ch if is_number(ch, false) => {
                    let number = self.number();
                    tokens.push(self.spanned(number));
                }
                '=' => {
                    if self.code[self.index + 1] == '=' {
                        let operator = self.operator();
                        tokens.push(self.spanned(operator));
                    } else {
                        let special = self.special();
                        tokens.push(self.spanned(special));
                    }
                }
                '#' => {
                    if self.code[self.index + 1] == ':' {
                        let doc_comment = self.doc_comment();
                        tokens.push(self.spanned(doc_comment));
                    } else {
                        self.comment();
                    }
                }
                ch if is_special(ch) => {
                    let special = self.special();
                    tokens.push(self.spanned(special));
                }
                '"' => {
                    let string = self.string()?;
                    tokens.push(self.spanned(string));
                }
                ch if is_op(ch) => {
                    let operator = self.operator();
                    tokens.push(self.spanned(operator));
                }
                ch if ch == '\n' => {
                    // token::proc doesn't matter, just needs to be
                    // something that doesn't trigger newline suppression
                    if tokens.len() > 0 && tokens.last().unwrap().contents == Token::Newline {
                        self.next(); // skip consecutive newlines
                    } else {
                        match tokens
                            .last()
                            .unwrap_or(&Span {
                                contents: Token::Proc,
                                pos: 0,
                                len: 0,
                            })
                            .contents
                        {
                            Token::Op(_) | Token::Comma => self.next(),
                            _ if self.nesting != 0 => self.next(),
                            _ => {
                                tokens.push(self.spanned(Token::Newline));
                                self.next()
                            }
                        };
                    }
                }
                ch if ch.is_ascii_whitespace() => {
                    self.next();
                }
                '\0' => break,
                _ => unreachable!(),
            }
        }
        Some(tokens)
    }

    fn spanned(&mut self, token: Token) -> Span<Token> {
        Span {
            contents: token.clone(),
            pos: self.index,
            len: token_len(&token),
        }
    }
}

#[inline]
fn is_ident(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[inline]
fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

#[inline]
fn is_number(ch: char, decimal_passed: bool) -> bool {
    ch.is_ascii_digit() || (ch == '.' && !decimal_passed)
}

#[inline]
fn is_special(ch: char) -> bool {
    SPECIAL_CHARS.contains(&ch)
}

#[inline]
fn is_op(ch: char) -> bool {
    ch.is_ascii_punctuation()
}

fn str_to_keyword(s: &str) -> Option<Token> {
    Some(match s {
        "proc" => Token::Proc,
        "if" => Token::If,
        "else" => Token::Else,
        "elif" => Token::Elif,
        "while" => Token::While,
        "loop" => Token::Loop,
        "var" => Token::Var,
        "const" => Token::Const,
        "return" => Token::Return,
        "use" => Token::Use,
        "break" => Token::Break,
        "continue" => Token::Continue,
        _ => return None,
    })
}

#[inline]
fn str_to_ident(s: &str) -> Token {
    Token::Ident(s.to_owned())
}

fn token_len(t: &Token) -> usize {
    match t {
        Token::IntLiteral(s) => s.len(),
        Token::FloatLiteral(s) => s.len(),
        Token::StrLiteral(s) => s.len(),

        Token::Ident(s) => s.len(),
        Token::Op(s) => s.len(),

        Token::DocComment(s) => s.len() + 2,

        Token::Proc => 4,
        Token::If => 2,
        Token::Else => 4,
        Token::Elif => 4,
        Token::While => 5,
        Token::Loop => 4,
        Token::Var => 3,
        Token::Const => 5,
        Token::Return => 6,
        Token::Use => 3,
        Token::Break => 5,
        Token::Continue => 8,

        Token::LParen
        | Token::RParen
        | Token::LBracket
        | Token::RBracket
        | Token::LBrace
        | Token::RBrace
        | Token::Comma
        | Token::Equals
        | Token::Colon => 1,

        // newline
        Token::Newline => 1,
        Token::EOF => unreachable!(),
    }
}
