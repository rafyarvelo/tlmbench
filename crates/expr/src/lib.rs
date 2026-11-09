//! TelemetryBench expressions: the small language behind derived channels, rules and checks.
//! `parse` turns text into a syntax tree, with errors that name the character; evaluation comes next.

pub mod ast;
pub mod eval;
pub mod filters;
pub mod functions;
pub mod lex;
pub mod parse;
pub mod series;

pub use ast::{BinOp, Expr, ExprError, UnOp};
pub use eval::{CHUNK, Env, Val, eval, eval_chunked, eval_str, run};
pub use parse::parse;
pub use series::{Grid, Series};

#[cfg(test)]
mod tests {
    use super::*;

    fn err(s: &str) -> ExprError {
        parse(s).unwrap_err()
    }

    #[test]
    fn precedence_and_unicode() {
        assert_eq!(parse("1 + 2 * 3").unwrap(), parse("1 + (2 * 3)").unwrap());
        assert_eq!(parse("x × 2 − 1").unwrap(), parse("x * 2 - 1").unwrap());
        assert_eq!(parse("2 ^ 3 ^ 2").unwrap(), parse("2 ^ (3 ^ 2)").unwrap());
        // unary minus binds looser than ^: -x ^ 2 is -(x ^ 2)
        let Expr::Un { op: UnOp::Neg, a } = parse("-x ^ 2").unwrap() else { panic!() };
        assert!(matches!(*a, Expr::Bin { op: BinOp::Pow, .. }));
    }

    #[test]
    fn names_labels_durations() {
        let e = parse("\"Close|Adj Close\" / 2 + sma(x, 30d) + (state == 'flight')").unwrap();
        assert_eq!(e.names(), ["Close|Adj Close", "x", "state"]);
        assert!(matches!(parse("5s").unwrap(), Expr::Dur { secs, .. } if secs == 5.0));
        assert!(matches!(parse("250ms").unwrap(), Expr::Dur { secs, .. } if (secs - 0.25).abs() < 1e-12));
        assert!(matches!(parse("1e3").unwrap(), Expr::Num(v) if v == 1000.0));
    }

    #[test]
    fn errors_name_the_character() {
        assert_eq!(err("x * (2 +"), ExprError::new("The expression ends too soon", 8));
        assert_eq!(err("foo(x)").msg, "There's no function called foo");
        assert_eq!(err("battery_v * (current_a"), ExprError::new("Expected \")\"", 22));
        assert_eq!(err("a # b").at, 2);
        assert_eq!(err("a b").at, 2);
        assert_eq!(err("'open").at, 0);
    }
}
