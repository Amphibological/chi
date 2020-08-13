//! The Elgin parser

use crate::lexer::Token;
use crate::errors::{Logger, Span};
use crate::types::Type;

#[derive(Debug, Clone)]
pub enum Node {
    Literal {
        typ: Type,
        value: String,
    },
    Call {
        name: String,
        args: Vec<Span<Node>>,
    },
    InfixOp {
        op: String,
        left: Box<Span<Node>>,
        right: Box<Span<Node>>,
    },
    PrefixOp {
        op: String,
        right: Box<Span<Node>>,
    },
    PostfixOp {
        op: String,
        left: Box<Span<Node>>,
    },
    IndexOp {
        object: Box<Span<Node>>,
        index: Box<Span<Node>>,
    },
    VariableRef {
        name: String,
    },
    IfStatement {
        condition: Box<Span<Node>>,
        body: Box<Span<Node>>,
        else_body: Box<Span<Node>>,
    },
    WhileStatement {
        condition: Box<Span<Node>>,
        body: Box<Span<Node>>,
    },
    Block {
        nodes: Vec<Span<Node>>,
    },
    VarStatement {
        name: String,
        typ: Type,
        value: Box<Span<Node>>,
    },
    ConstStatement {
        name: String,
        typ: Type,
        value: Box<Span<Node>>,
    },
    AssignStatement {
        name: String,
        value: Box<Span<Node>>,
    },
    ProcStatement {
        name: String,
        args: Vec<String>,
        arg_types: Vec<Type>,
        ret_type: Type,
        body: Box<Span<Node>>,
    },
    ReturnStatement {
        val: Box<Span<Node>>,
    },
}

fn spanned(node: Node, pos: usize, len: usize) -> Span<Node> {
    Span {
        contents: node.clone(),
        pos,
        len,
    }
}

pub struct Parser<'p> {
    tokens: &'p [Span<Token>],
    index: usize,
}

impl<'p> Parser<'p> {
    pub fn new(tokens: &'p [Span<Token>]) -> Self {
        Parser { 
            tokens, 
            index: 0,
        }
    }

    fn next(&mut self) -> Span<Token> {
        self.index += 1;
        if self.index >= self.tokens.len() {
            let last = self.tokens.last().unwrap();
            return Span {
                contents: Token::EOF,
                pos: last.pos,
                len: last.len,
            };
        }
        self.tokens[self.index - 1].clone() }
    fn peek(&mut self) -> Span<Token> {
        if self.index >= self.tokens.len() {
            let last = self.tokens.last().unwrap();
            return Span {
                contents: Token::EOF,
                pos: last.pos,
                len: last.len,
            };
        }
        self.tokens[self.index].clone()
    }

    fn ensure_next(&mut self, t: Token) -> Option<()> {
        if self.peek().contents == t {
            self.next();
            Some(())
        } else {
            Logger::syntax_error(
                format!("Expected a {:?} token, but found a {:?} instead", t, self.peek().contents.clone()).as_str(),
                self.peek().pos,
                self.peek().len,
            );
            None
        }
    }

    fn ensure_ident(&mut self) -> Option<String> {
        if let Token::Ident(id) = self.peek().contents.clone() {
            self.next();
            Some(id)
        } else {
            Logger::syntax_error(
                format!("Expected an identifier, but found a {:?} token instead", self.peek().contents.clone()).as_str(),
                self.peek().pos,
                self.peek().len,
            );
            None
        }
    }

    fn ensure_type(&mut self) -> Option<Type> {
        match self.peek().contents.clone() {
            Token::Ident(id) => {
                let typ = match id.as_str() {
                    "i8" => Type::I8,
                    "i16" => Type::I16,
                    "i32" => Type::I32,
                    "i64" => Type::I64,
                    "i128" => Type::I128,

                    "n8" => Type::N8,
                    "n16" => Type::N16,
                    "n32" => Type::N32,
                    "n64" => Type::N64,
                    "n128" => Type::N128,

                    "f32" => Type::F32,
                    "f64" => Type::F64,
                    "f128" => Type::F128,

                    "bool" => Type::Bool,

                    _ => {
                        Logger::syntax_error(
                            format!("Expected a type, but found a {:?} instead", self.peek().contents.clone()).as_str(),
                            self.peek().pos,
                            self.peek().len,
                        );
                        return None
                    }
                };
                self.next();
                Some(typ)
            },
            Token::Op(s) if s == "*" => {
                self.next();
                let content_type = self.ensure_type()?;
                Some(Type::Ptr(Box::new(content_type)))
            },
            Token::LBracket => {
                self.next(); // skip the LBracket
                if let Token::IntLiteral(size) = self.peek().contents {
                    self.next();
                    self.ensure_next(Token::RBracket)?;
                    let content_type = self.ensure_type()?; 
                    Some(Type::Array(size.parse().unwrap(), Box::new(content_type)))
                } else {
                    Logger::syntax_error(
                        format!("Expect an integer as the length of an array, but found a {:?} token instead", self.peek().contents).as_str(),
                        self.peek().pos,
                        self.peek().len,
                    );
                    None
                }
            },
            _ => {
                Logger::syntax_error(
                    format!("Expected a type, but found a {:?} instead", self.peek().contents.clone()).as_str(),
                    self.peek().pos,
                    self.peek().len,
                );
                None
            },
        }
    }

    pub fn go(&mut self) -> Option<Vec<Span<Node>>> {
        let mut nodes = vec![];
        loop {
            match self.peek().contents {
                Token::DocComment(_) => {
                    self.next(); // one day there will be doc comment support
                },
                Token::Newline => {
                    self.next();
                },
                _ => {
                    nodes.push(self.statement()?);
                    self.ensure_next(Token::Newline)?;
                }
            };
            if self.peek().contents == Token::EOF {
                break;
            }
        }
        Some(nodes)
    }

    fn statement(&mut self) -> Option<Span<Node>> {
        Some(match self.peek().contents {
            Token::If => self.if_statement(true)?,
            Token::While => self.while_statement()?,
            Token::Loop => self.loop_statement()?,
            Token::Var => self.var_statement()?,
            Token::Const => self.const_statement()?,
            Token::Proc => self.proc_statement()?,
            Token::Return => self.return_statement()?,
            Token::Ident(_) if self.tokens[self.index + 1].contents == Token::Equals => {
                self.assign_statement()?
            }
            _ => self.expr(0)?,
        })
    }

    fn if_statement(&mut self, ensure_if: bool) -> Option<Span<Node>> {
        if ensure_if {
            self.ensure_next(Token::If)?;
        }
        let condition = self.expr(0)?;
        let body = self.block()?;
        let else_body;
        if self.peek().contents == Token::Elif {
            self.ensure_next(Token::Elif)?;
            else_body = self.if_statement(false)?;
        } else if self.peek().contents == Token::Else {
            self.ensure_next(Token::Else)?;
            else_body = self.block()?;
        } else {
            else_body = spanned(Node::Block {
                nodes: vec![
                    spanned(Node::Literal {
                        typ: Type::Undefined,
                        value: "undefined".to_owned(),
                    }, 0, 0)
                ],
            }, 0, 0);
        }

        Some(spanned(Node::IfStatement {
            condition: Box::new(condition),
            body: Box::new(body.clone()),
            else_body: Box::new(else_body),
        }, 0, 0))
    }

    fn while_statement(&mut self) -> Option<Span<Node>> {
        self.ensure_next(Token::While)?;
        let condition = self.expr(0)?;
        let body = self.block()?;

        Some(spanned(Node::WhileStatement {
            condition: Box::new(condition),
            body: Box::new(body.clone()),
        }, 0, 0))
    }

    fn loop_statement(&mut self) -> Option<Span<Node>> {
        self.ensure_next(Token::Loop)?;
        let condition = spanned(Node::Literal {
            typ: Type::Bool,
            value: "true".to_owned(),
        }, 0, 0);
        let body = self.block()?;

        Some(spanned(Node::WhileStatement {
            condition: Box::new(condition),
            body: Box::new(body.clone()),
        }, 0, 0))
    }

    fn block(&mut self) -> Option<Span<Node>> {
        let mut nodes = vec![];
        self.ensure_next(Token::LBrace)?;
        loop {
            let _ = self.ensure_next(Token::Newline);
            nodes.push(self.statement()?);
            if self.ensure_next(Token::Newline).is_none() {
                self.ensure_next(Token::RBrace)?;
                break;
            }
            if self.peek().contents == Token::RBrace {
                self.ensure_next(Token::RBrace)?;
                break;
            }
        }
        Some(spanned(Node::Block {
            nodes,
        }, 0, 0))
    }

    fn var_statement(&mut self) -> Option<Span<Node>> {
        self.ensure_next(Token::Var)?;
        let name = self.ensure_ident()?;
        let typ;
        if self.ensure_next(Token::Colon).is_none() {
            typ = self.ensure_type()?;
        } else {
            typ = Type::Unknown;
        }
        let value;
        if self.peek().contents == Token::Equals {
            self.ensure_next(Token::Equals)?;
            value = self.expr(0)?;
        } else {
            value = spanned(Node::Literal {
                typ: Type::Undefined,
                value: "undefined".to_owned(),
            }, 0, 0);
        }

        Some(spanned(Node::VarStatement {
            name,
            typ,
            value: Box::new(value),
        }, 0, 0))
    }

    fn assign_statement(&mut self) -> Option<Span<Node>> {
        let name = self.ensure_ident()?;
        self.ensure_next(Token::Equals)?;
        let value = self.expr(0)?;

        Some(spanned(Node::AssignStatement {
            name,
            value: Box::new(value),
        }, 0, 0))
    }

    fn const_statement(&mut self) -> Option<Span<Node>> {
        self.ensure_next(Token::Const)?;
        let name = self.ensure_ident()?;
        let typ;
        if self.ensure_next(Token::Colon).is_some() {
            typ = self.ensure_type()?;
        } else {
            typ = Type::Unknown;
        }
        self.ensure_next(Token::Equals)?;
        let value = self.expr(0)?;

        Some(spanned(Node::ConstStatement {
            name,
            typ,
            value: Box::new(value),
        }, 0, 0))
    }

    fn proc_statement(&mut self) -> Option<Span<Node>> {
        self.ensure_next(Token::Proc)?;
        let name = self.ensure_ident()?;
        self.ensure_next(Token::LParen)?;
        let mut args = vec![];
        let mut arg_types = vec![];
        while self.peek().contents != Token::RParen {
            args.push(self.ensure_ident()?);
            self.ensure_next(Token::Colon)?;
            arg_types.push(self.ensure_type()?);
            if self.peek().contents != Token::Comma {
                break;
            } else {
                self.ensure_next(Token::Comma)?;
            }
        }
        self.ensure_next(Token::RParen)?;
        let ret_type;
        if self.ensure_next(Token::Colon).is_some() {
            ret_type = self.ensure_type()?;
        } else {
            ret_type = Type::Undefined;
        }
        let body;
        if self.peek().contents == Token::LBrace {
            body = self.block()?;
        } else {
            body = spanned(Node::Block {
                nodes: vec![],
            }, 0, 0);
        }

        Some(spanned(Node::ProcStatement {
            name,
            args,
            arg_types,
            ret_type,
            body: Box::new(body),
        }, 0, 0))
    }

    fn return_statement(&mut self) -> Option<Span<Node>> {
        self.ensure_next(Token::Return)?;
        let val = self.expr(0)?;
        Some(spanned(Node::ReturnStatement {
            val: Box::new(val),
        }, 0, 0))
    }

    fn expr(&mut self, min_bp: u8) -> Option<Span<Node>> {
        let mut left = match self.next().clone() {
            Span {
                contents: Token::Ident(id),
                pos,
                len,
            } => {
                if self.peek().contents == Token::LParen {
                    self.next(); // pass the LParen;
                    let mut args = Vec::new();
                    while self.peek().contents != Token::RParen {
                        args.push(self.expr(0)?);
                        if self.peek().contents != Token::Comma {
                            break;
                        } else {
                            self.ensure_next(Token::Comma)?;
                        }
                    }
                    self.ensure_next(Token::RParen)?;
                    spanned(Node::Call {
                        name: id,
                        args,
                    }, pos, len)
                } else {
                    spanned(Node::VariableRef {
                        name: id,
                    }, pos, len)
                }
            }
            Span {
                contents: Token::IntLiteral(int),
                pos,
                len,
            } => spanned(Node::Literal {
                typ: Type::IntLiteral,
                value: int,
            }, pos, len),
            Span {
                contents: Token::FloatLiteral(float),
                pos,
                len,
            } => spanned(Node::Literal {
                typ: Type::FloatLiteral,
                value: float,
            }, pos, len),
            Span {
                contents: Token::StrLiteral(s),
                pos,
                len,
            } => spanned(Node::Literal {
                typ: Type::StrLiteral,
                value: s,
            }, pos, len),
            Span {
                contents: Token::LParen,
                ..
            } => {
                let left = self.expr(0)?;
                self.ensure_next(Token::RParen)?;
                left
            }
            Span {
                contents: Token::Op(op),
                pos,
                len,
            } => {
                let ((), right_bp) = prefix_binding_power(&op);
                let right = self.expr(right_bp)?;
                spanned(Node::PrefixOp {
                    op,
                    right: Box::new(right),
                }, pos, len)
            }
            Span {
                contents: Token::EOF,
                pos,
                len,
            } => {
                Logger::syntax_error("Encountered the end of the file while parsing", pos, len);
                return None
            }
            t => panic!("Bad token: {:?}", t),
        };

        loop {
            let op = match self.peek().contents.clone() {
                Token::EOF
                | Token::Newline
                | Token::RParen
                | Token::RBracket
                | Token::Comma
                | Token::LBrace
                | Token::RBrace => break,
                Token::Op(op) => op,
                Token::LBracket => "[".to_owned(),
                t => panic!("Bad token: {:?}", t),
            };

            if let Some((left_bp, ())) = postfix_binding_power(&op) {
                if left_bp < min_bp {
                    break;
                }
                self.next();

                left = if op == "[" {
                    let right = self.expr(0)?;
                    self.ensure_next(Token::RBracket)?;
                    spanned(Node::IndexOp {
                        object: Box::new(left),
                        index: Box::new(right),
                    }, 0, 0)
                } else {
                    spanned(Node::PostfixOp {
                        op,
                        left: Box::new(left),
                    }, 0, 0)
                };
                continue;
            }

            if let Some((left_bp, right_bp)) = infix_binding_power(&op) {
                if left_bp < min_bp {
                    break;
                }
                self.next();

                let right = self.expr(right_bp)?;
                left = spanned(Node::InfixOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                }, 0, 0);
                continue;
            }

            break;
        }

        Some(left)
    }
}

fn prefix_binding_power(op: &String) -> ((), u8) {
    match op.as_str() {
        "!" => ((), 8),
        "+" | "-" => ((), 9),
        o => unreachable!(o),
    }
}

fn postfix_binding_power(op: &String) -> Option<(u8, ())> {
    Some(match op.as_str() {
        "[" => (11, ()),
        _ => return None,
    })
}

fn infix_binding_power(op: &String) -> Option<(u8, u8)> {
    Some(match op.as_str() {
        ">" | "<" | ">=" | "<=" | "==" | "!=" => (3, 4),
        "+" | "-" => (5, 6),
        "*" | "/" => (7, 8),
        _ => return None,
    })
}
