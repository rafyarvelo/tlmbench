//! The syntax tree.

use thiserror::Error;

/// A problem in an expression's text: a message and the character (counted in characters) it points at.
#[derive(Clone, Debug, PartialEq, Error)]
#[error("{msg} (at {at})")]
pub struct ExprError {
    pub msg: String,
    pub at: usize,
}

impl ExprError {
    pub fn new(msg: impl Into<String>, at: usize) -> Self {
        Self { msg: msg.into(), at }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Or,
    And,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    /// A duration in seconds, written `30d`, `5s`, `250ms`; `text` is as typed.
    Dur {
        secs: f64,
        text: String,
    },
    /// A value label in single quotes: `state == 'flight'`.
    Label {
        text: String,
        at: usize,
    },
    /// A channel name, bare or quoted; may hold alternatives `a|b`.
    Name {
        name: String,
        at: usize,
    },
    Un {
        op: UnOp,
        a: Box<Expr>,
    },
    Bin {
        op: BinOp,
        a: Box<Expr>,
        b: Box<Expr>,
    },
    Call {
        f: String,
        args: Vec<Expr>,
        at: usize,
    },
}

impl Expr {
    /// A copy with each name `f(name, position)` answers for replaced by what it returns.
    pub fn map_names(&self, f: &mut dyn FnMut(&str, usize) -> Option<Expr>) -> Expr {
        match self {
            Expr::Name { name, at } => f(name, *at).unwrap_or_else(|| self.clone()),
            Expr::Un { op, a } => Expr::Un { op: *op, a: Box::new(a.map_names(f)) },
            Expr::Bin { op, a, b } => Expr::Bin { op: *op, a: Box::new(a.map_names(f)), b: Box::new(b.map_names(f)) },
            Expr::Call { f: name, args, at } => {
                Expr::Call { f: name.clone(), args: args.iter().map(|a| a.map_names(f)).collect(), at: *at }
            }
            _ => self.clone(),
        }
    }

    /// The channel names the expression reads, in order of first use.
    pub fn names(&self) -> Vec<&str> {
        fn walk<'a>(e: &'a Expr, out: &mut Vec<&'a str>) {
            match e {
                Expr::Name { name, .. } if !out.contains(&name.as_str()) => out.push(name),
                Expr::Un { a, .. } => walk(a, out),
                Expr::Bin { a, b, .. } => {
                    walk(a, out);
                    walk(b, out);
                }
                Expr::Call { args, .. } => args.iter().for_each(|a| walk(a, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        walk(self, &mut out);
        out
    }
}
