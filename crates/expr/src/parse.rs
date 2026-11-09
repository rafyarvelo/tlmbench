//! Precedence-climbing parser. Function names are checked against `functions::arity`.

use crate::ast::{BinOp, Expr, ExprError, UnOp};
use crate::lex::{Tok, Token, lex};

fn binary(op: &str) -> Option<(BinOp, u8, bool)> {
    // (operator, precedence, right-associative)
    Some(match op {
        "||" => (BinOp::Or, 1, false),
        "&&" => (BinOp::And, 2, false),
        "<" => (BinOp::Lt, 3, false),
        "<=" => (BinOp::Le, 3, false),
        ">" => (BinOp::Gt, 3, false),
        ">=" => (BinOp::Ge, 3, false),
        "==" => (BinOp::Eq, 3, false),
        "!=" => (BinOp::Ne, 3, false),
        "+" => (BinOp::Add, 4, false),
        "-" => (BinOp::Sub, 4, false),
        "*" => (BinOp::Mul, 5, false),
        "/" => (BinOp::Div, 5, false),
        "%" => (BinOp::Rem, 5, false),
        "^" => (BinOp::Pow, 7, true),
        _ => return None,
    })
}

struct Parser {
    t: Vec<Token>,
    p: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.t[self.p]
    }
    fn next(&mut self) -> Token {
        let t = self.t[self.p].clone();
        if self.p + 1 < self.t.len() {
            self.p += 1;
        }
        t
    }
    fn is_op(&self, o: &str) -> bool {
        matches!(&self.peek().tok, Tok::Op(x) if *x == o)
    }
    fn expect(&mut self, o: &str) -> Result<(), ExprError> {
        let t = self.next();
        match t.tok {
            Tok::Op(x) if x == o => Ok(()),
            _ => Err(ExprError::new(format!("Expected \"{o}\""), t.at)),
        }
    }

    fn prefix(&mut self) -> Result<Expr, ExprError> {
        let t = self.next();
        match t.tok {
            Tok::Num(v) => Ok(Expr::Num(v)),
            Tok::Dur { secs, text } => Ok(Expr::Dur { secs, text }),
            Tok::Label(text) => Ok(Expr::Label { text, at: t.at }),
            Tok::Name(name) => Ok(match name.as_str() {
                "pi" => Expr::Num(std::f64::consts::PI),
                "e" => Expr::Num(std::f64::consts::E),
                _ => Expr::Name { name, at: t.at },
            }),
            Tok::Func(f) => {
                self.expect("(")?;
                let mut args = Vec::new();
                if !self.is_op(")") {
                    loop {
                        args.push(self.expr(0)?);
                        if self.is_op(",") {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(")")?;
                let f = f.to_lowercase();
                if !crate::functions::exists(&f) {
                    return Err(ExprError::new(format!("There's no function called {f}"), t.at));
                }
                Ok(Expr::Call { f, args, at: t.at })
            }
            Tok::Op("(") => {
                let e = self.expr(0)?;
                self.expect(")")?;
                Ok(e)
            }
            Tok::Op("-") => Ok(Expr::Un { op: UnOp::Neg, a: Box::new(self.expr(6)?) }),
            Tok::Op("!") => Ok(Expr::Un { op: UnOp::Not, a: Box::new(self.expr(6)?) }),
            Tok::Op("+") => self.expr(6),
            Tok::End => Err(ExprError::new("The expression ends too soon", t.at)),
            Tok::Op(o) => Err(ExprError::new(format!("Unexpected \"{o}\""), t.at)),
        }
    }

    fn expr(&mut self, min: u8) -> Result<Expr, ExprError> {
        let mut left = self.prefix()?;
        while let Tok::Op(o) = &self.peek().tok {
            let Some((op, prec, right)) = binary(o) else { break };
            if prec < min {
                break;
            }
            self.next();
            let b = self.expr(if right { prec } else { prec + 1 })?;
            left = Expr::Bin { op, a: Box::new(left), b: Box::new(b) };
        }
        Ok(left)
    }
}

/// Parses `src`. Errors carry the character they point at.
pub fn parse(src: &str) -> Result<Expr, ExprError> {
    let mut p = Parser { t: lex(src)?, p: 0 };
    let e = p.expr(0)?;
    match &p.peek().tok {
        Tok::End => Ok(e),
        Tok::Op(o) => Err(ExprError::new(format!("Unexpected \"{o}\""), p.peek().at)),
        _ => Err(ExprError::new("Unexpected name or number", p.peek().at)),
    }
}
