//! レキサー。エラーがあってもトークン列の生成を続け、
//! 発生したエラーは [`SyntaxError`] として併せて返す(エラー回復)。

use crate::error::{codes, FixHint, SyntaxError};
use crate::span::{Position, Span};
use crate::token::{Token, TokenKind};

pub fn lex(source: &str) -> (Vec<Token>, Vec<SyntaxError>) {
    let mut lexer = Lexer::new(source);
    lexer.run();
    (lexer.tokens, lexer.errors)
}

struct Lexer {
    chars: Vec<char>,
    idx: usize,
    line: u32,
    col: u32,
    tokens: Vec<Token>,
    errors: Vec<SyntaxError>,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            idx: 0,
            line: 1,
            col: 1,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn pos(&self) -> Position {
        Position::new(self.line, self.col)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.idx).copied()
    }

    fn peek2(&self) -> Option<char> {
        self.chars.get(self.idx + 1).copied()
    }

    fn bump(&mut self) -> char {
        let c = self.chars[self.idx];
        self.idx += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        c
    }

    fn push(&mut self, kind: TokenKind, text: impl Into<String>, start: Position) {
        self.tokens.push(Token {
            kind,
            text: text.into(),
            span: Span::new(start, self.pos()),
        });
    }

    fn run(&mut self) {
        while let Some(c) = self.peek() {
            let start = self.pos();
            match c {
                ' ' | '\t' | '\r' => {
                    self.bump();
                }
                '\n' => {
                    self.bump();
                    self.push(TokenKind::Newline, "\n", start);
                }
                '/' if self.peek2() == Some('/') => {
                    self.bump(); // '/'
                    self.bump(); // '/'
                    let mut text = String::new();
                    while self.peek().is_some_and(|c| c != '\n') {
                        text.push(self.bump());
                    }
                    self.push(TokenKind::Comment, text, start);
                }
                c if c.is_ascii_alphabetic() || c == '_' => self.ident(),
                c if c.is_ascii_digit() => self.number(),
                '"' => self.string(),
                _ => self.punct(),
            }
        }
        let eof = self.pos();
        self.push(TokenKind::Eof, "", eof);
    }

    fn ident(&mut self) {
        let start = self.pos();
        let mut text = String::new();
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            text.push(self.bump());
        }
        let kind = TokenKind::keyword(&text).unwrap_or(TokenKind::Ident);
        self.push(kind, text, start);
    }

    fn number(&mut self) {
        let start = self.pos();
        let mut text = String::new();
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            text.push(self.bump());
        }
        let span = Span::new(start, self.pos());
        if text.parse::<i64>().is_err() {
            self.errors.push(SyntaxError {
                code: codes::INT_OUT_OF_RANGE,
                message: format!("integer literal '{text}' is out of range for 64-bit integers"),
                span,
                fix: FixHint::direction("Use a smaller integer value"),
            });
        }
        self.push(TokenKind::Int, text, start);
    }

    fn string(&mut self) {
        let start = self.pos();
        self.bump(); // 開始の '"'
        let mut value = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    let here = self.pos();
                    self.errors.push(SyntaxError {
                        code: codes::UNTERMINATED_STRING,
                        message: "unterminated string literal".to_string(),
                        span: Span::new(start, here),
                        fix: FixHint::replace("Insert a closing '\"'", Span::point(here), "\""),
                    });
                    break;
                }
                Some('"') => {
                    self.bump();
                    break;
                }
                Some('\\') => {
                    let esc_start = self.pos();
                    self.bump();
                    match self.peek() {
                        Some('n') => {
                            self.bump();
                            value.push('\n');
                        }
                        Some('t') => {
                            self.bump();
                            value.push('\t');
                        }
                        Some('r') => {
                            self.bump();
                            value.push('\r');
                        }
                        Some('\\') => {
                            self.bump();
                            value.push('\\');
                        }
                        Some('"') => {
                            self.bump();
                            value.push('"');
                        }
                        Some(other) if other != '\n' => {
                            self.bump();
                            let span = Span::new(esc_start, self.pos());
                            self.errors.push(SyntaxError {
                                code: codes::INVALID_ESCAPE,
                                message: format!("invalid escape sequence '\\{other}'"),
                                span,
                                fix: FixHint::replace(
                                    "Escape the backslash itself",
                                    span,
                                    format!("\\\\{other}"),
                                ),
                            });
                            value.push(other);
                        }
                        // 行末・EOF 直前のバックスラッシュは未終端処理に任せる
                        _ => {}
                    }
                }
                Some(c) => {
                    self.bump();
                    value.push(c);
                }
            }
        }
        self.push(TokenKind::Str, value, start);
    }

    fn punct(&mut self) {
        let start = self.pos();
        let c = self.bump();
        let two = |second: char, this: &Self| this.peek() == Some(second);
        let kind = match c {
            '-' if two('>', self) => {
                self.bump();
                TokenKind::Arrow
            }
            '=' if two('=', self) => {
                self.bump();
                TokenKind::EqEq
            }
            '=' if two('>', self) => {
                self.bump();
                TokenKind::FatArrow
            }
            '!' if two('=', self) => {
                self.bump();
                TokenKind::NotEq
            }
            '<' if two('=', self) => {
                self.bump();
                TokenKind::Le
            }
            '>' if two('=', self) => {
                self.bump();
                TokenKind::Ge
            }
            '|' if two('|', self) => {
                self.bump();
                TokenKind::OrOr
            }
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            '.' => TokenKind::Dot,
            '=' => TokenKind::Eq,
            '<' => TokenKind::Lt,
            '>' => TokenKind::Gt,
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '!' => TokenKind::Bang,
            other => {
                let span = Span::new(start, self.pos());
                self.errors.push(SyntaxError {
                    code: codes::UNEXPECTED_CHAR,
                    message: format!("unexpected character '{other}'"),
                    span,
                    fix: FixHint::replace("Remove this character", span, ""),
                });
                return;
            }
        };
        let text = kind.literal().expect("punct tokens have literal text");
        self.push(kind, text, start);
    }
}
