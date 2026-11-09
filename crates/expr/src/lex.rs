//! Tokens: numbers, durations (30d, 5s, 250ms), names (bare or quoted), 'labels', operators.

use crate::ast::ExprError;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Num(f64),
    Dur {
        secs: f64,
        text: String,
    },
    Label(String),
    Name(String),
    /// A name followed by `(`.
    Func(String),
    Op(&'static str),
    End,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    /// Character position.
    pub at: usize,
}

const UNITS: [(&str, f64); 6] =
    [("ms", 0.001), ("min", 60.0), ("wk", 604800.0), ("s", 1.0), ("h", 3600.0), ("d", 86400.0)];
const TWO: [&str; 6] = ["<=", ">=", "==", "!=", "&&", "||"];
const ONE: [(char, &str); 10] = [
    ('+', "+"),
    ('-', "-"),
    ('*', "*"),
    ('/', "/"),
    ('^', "^"),
    ('%', "%"),
    ('(', "("),
    (')', ")"),
    (',', ","),
    ('<', "<"),
];

pub fn lex(src: &str) -> Result<Vec<Token>, ExprError> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        let at = i;
        let digit_at = |k: usize| c.get(k).is_some_and(char::is_ascii_digit);
        if ch.is_ascii_digit() || (ch == '.' && digit_at(i + 1)) {
            while digit_at(i) {
                i += 1;
            }
            if c.get(i) == Some(&'.') {
                i += 1;
                while digit_at(i) {
                    i += 1;
                }
            }
            if matches!(c.get(i), Some('e' | 'E')) {
                let mut j = i + 1;
                if matches!(c.get(j), Some('+' | '-')) {
                    j += 1;
                }
                if digit_at(j) {
                    while digit_at(j) {
                        j += 1;
                    }
                    i = j;
                }
            }
            let text: String = c[at..i].iter().collect();
            let v: f64 = text.parse().map_err(|_| ExprError::new(format!("\"{text}\" isn't a number"), at))?;
            let rest: String = c[i..].iter().collect();
            let unit = UNITS.iter().find(|(u, _)| {
                rest.starts_with(u)
                    && !rest[u.len()..].chars().next().is_some_and(|n| n.is_ascii_alphanumeric() || n == '_')
            });
            match unit {
                Some((u, k)) => {
                    i += u.len();
                    out.push(Token { tok: Tok::Dur { secs: v * k, text: format!("{text}{u}") }, at });
                }
                None => out.push(Token { tok: Tok::Num(v), at }),
            }
            continue;
        }
        if ch == '\'' {
            let j = c[i + 1..]
                .iter()
                .position(|&x| x == '\'')
                .ok_or_else(|| ExprError::new("A value name in single quotes is missing its closing quote", at))?;
            out.push(Token { tok: Tok::Label(c[i + 1..i + 1 + j].iter().collect()), at });
            i += j + 2;
            continue;
        }
        if ch == '"' || ch == '`' {
            let j = c[i + 1..]
                .iter()
                .position(|&x| x == ch)
                .ok_or_else(|| ExprError::new("A name in quotes is missing its closing quote", at))?;
            out.push(Token { tok: Tok::Name(c[i + 1..i + 1 + j].iter().collect()), at });
            i += j + 2;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            while c.get(i).is_some_and(|x| x.is_ascii_alphanumeric() || *x == '_' || *x == '.') {
                i += 1;
            }
            let name: String = c[at..i].iter().collect();
            let call = c[i..].iter().find(|x| !x.is_whitespace()) == Some(&'(');
            out.push(Token { tok: if call { Tok::Func(name) } else { Tok::Name(name) }, at });
            continue;
        }
        let two: String = c[i..(i + 2).min(c.len())].iter().collect();
        if let Some(op) = TWO.iter().find(|o| **o == two) {
            out.push(Token { tok: Tok::Op(op), at });
            i += 2;
            continue;
        }
        let op = match ch {
            '×' => Some("*"),
            '÷' => Some("/"),
            '−' => Some("-"),
            '>' => Some(">"),
            '!' => Some("!"),
            _ => ONE.iter().find(|(k, _)| *k == ch).map(|(_, o)| *o),
        };
        match op {
            Some(o) => {
                out.push(Token { tok: Tok::Op(o), at });
                i += 1;
            }
            None => return Err(ExprError::new(format!("\"{ch}\" isn't something an expression can use"), at)),
        }
    }
    out.push(Token { tok: Tok::End, at: c.len() });
    Ok(out)
}
