//! The function table: names and argument counts (min, max). Evaluation arrives with the evaluator.

/// (name, fewest arguments, most arguments)
pub const TABLE: &[(&str, usize, usize)] = &[
    // math
    ("abs", 1, 1),
    ("sqrt", 1, 1),
    ("ln", 1, 1),
    ("log", 1, 1),
    ("log10", 1, 1),
    ("exp", 1, 1),
    ("pow", 2, 2),
    ("floor", 1, 1),
    ("ceil", 1, 1),
    ("sign", 1, 1),
    ("round", 1, 2),
    ("clamp", 3, 3),
    ("sin", 1, 1),
    ("cos", 1, 1),
    ("tan", 1, 1),
    ("asin", 1, 1),
    ("acos", 1, 1),
    ("atan", 1, 1),
    ("atan2", 2, 2),
    ("deg", 1, 1),
    ("rad", 1, 1),
    ("if", 3, 3),
    ("min", 1, 9),
    ("max", 1, 9),
    // sample to sample
    ("delta", 1, 1),
    ("diff", 1, 1),
    ("prev", 1, 2),
    ("pct_change", 1, 1),
    ("rate", 1, 1),
    ("cumsum", 1, 1),
    ("integral", 1, 1),
    ("running_max", 1, 1),
    ("running_min", 1, 1),
    // rolling
    ("sma", 1, 2),
    ("rolling_mean", 1, 2),
    ("rolling_std", 1, 2),
    ("rolling_min", 1, 2),
    ("rolling_max", 1, 2),
    ("rolling_sum", 1, 2),
    ("ema", 1, 2),
    // filters
    ("movavg", 1, 2),
    ("median", 1, 2),
    ("lowpass", 2, 2),
    ("highpass", 2, 2),
    ("bandpass", 3, 3),
    ("interp", 1, 1),
    // one number
    ("mean", 1, 1),
    ("sum", 1, 1),
    ("std", 1, 1),
    ("first", 1, 1),
    ("last", 1, 1),
    ("count", 1, 1),
    ("pct", 1, 1),
    ("duration", 1, 1),
    ("occurrences", 1, 1),
    ("since", 1, 1),
    // limits
    ("past_warn", 1, 1),
    ("past_alarm", 1, 1),
];

pub fn exists(name: &str) -> bool {
    TABLE.iter().any(|(n, ..)| *n == name)
}

pub fn arity(name: &str) -> Option<(usize, usize)> {
    TABLE.iter().find(|(n, ..)| *n == name).map(|&(_, lo, hi)| (lo, hi))
}
