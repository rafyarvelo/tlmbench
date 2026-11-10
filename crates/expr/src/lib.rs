//! TelemetryBench expressions: the small language behind derived channels, rules and checks.
//! `parse` turns text into a syntax tree, with errors that name the character; evaluation comes next.

pub mod ast;
pub mod eval;
pub mod fft;
pub mod filters;
pub mod functions;
pub mod lex;
pub mod parse;
pub mod recipe;
pub mod series;

pub use ast::{BinOp, Expr, ExprError, UnOp};
pub use eval::{
    CHUNK, Env, Val, eval, eval_chunked, eval_range, eval_str, eval_warm, is_stateless, run, warm_rows, warm_rows_at,
};
pub use fft::{Spectrum, spectrum};
pub use parse::parse;
pub use recipe::{Recipe, Resolved};
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
    fn recipes_use_recipes_and_files() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/recordings/.tlmbench/recipes/power.toml"
        ))
        .unwrap();
        let r = recipe::from_toml(&text).unwrap();
        assert_eq!((r[0].name.as_str(), r[0].decimals, r[0].automatic), ("Power", Some(1), true));
        assert_eq!(recipe::from_toml(&recipe::to_toml(&r)).unwrap(), r);
        let mk = |n: &str, e: &str| Recipe {
            name: n.into(),
            expr: e.into(),
            unit: None,
            category: None,
            decimals: None,
            automatic: true,
        };
        let has = |n: &str| ["v", "i"].contains(&n);
        let rs = [
            mk("P", "v * i"),
            mk("Q", "sma(P, 5) + 1"),
            mk("Loop", "Loop2"),
            mk("Loop2", "Loop"),
            mk("Bad", "nope + 1"),
        ];
        let (ok, failed) = recipe::resolve(&rs, &has);
        assert_eq!(ok.iter().map(|r| r.recipe.name.as_str()).collect::<Vec<_>>(), ["P", "Q"]);
        assert_eq!(ok[1].expr.names(), ["v", "i"]);
        assert!(failed.iter().any(|(n, e)| n == "Loop" && e.msg.contains("uses itself")));
        assert!(failed.iter().any(|(n, e)| n == "Bad" && e.msg == "There's no channel called nope"));
        // editing a recipe a recipe uses changes the dependant's key
        let (again, _) = recipe::resolve(&[mk("P", "v / i"), rs[1].clone()], &has);
        assert_ne!(ok[1].key(), again[1].key());
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
