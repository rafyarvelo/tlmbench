//! Table sort and filter over the whole column: the filter language (`> 1.5`, `1..2`, `blank`, joined with and/or),
//! a multi-column sort, and the row-id list a table pages through. Pure: columns come in through a reader, so the
//! store can feed it from a recording and tests from vectors.

use rayon::prelude::*;
use std::cmp::Ordering;
use std::ops::Range;

use crate::time::Timestamp;

/// One test on a value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cond {
    Gt(f64),
    Ge(f64),
    Lt(f64),
    Le(f64),
    Eq(f64),
    Ne(f64),
    /// `lo..hi`, ends included
    Range(f64, f64),
    Blank,
    NotBlank,
}

impl Cond {
    pub fn test(self, v: f64) -> bool {
        let tol = |a: f64| (a.abs() * 1e-6).max(1e-9);
        match self {
            Cond::Gt(a) => v > a,
            Cond::Ge(a) => v >= a,
            Cond::Lt(a) => v < a,
            Cond::Le(a) => v <= a,
            Cond::Eq(a) => (v - a).abs() <= tol(a),
            Cond::Ne(a) => !v.is_nan() && (v - a).abs() > tol(a),
            Cond::Range(a, b) => v >= a.min(b) && v <= a.max(b),
            Cond::Blank => v.is_nan(),
            Cond::NotBlank => !v.is_nan(),
        }
    }
}

/// A column's filter: groups joined by `or`, each a list of conditions joined by `and`.
#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    pub groups: Vec<Vec<Cond>>,
}

impl Filter {
    /// `Ok(None)` for empty text (no filter); an error names the part that isn't understood.
    pub fn parse(text: &str) -> Result<Option<Filter>, String> {
        let s = text.trim().to_lowercase();
        if s.is_empty() {
            return Ok(None);
        }
        let mut groups = Vec::new();
        for or in split_on(&s, &[" or ", "||"]) {
            let mut group = Vec::new();
            for and in split_on(&or, &[" and ", "&&"]) {
                group.push(atom(and.trim()).ok_or_else(|| format!("can't read \"{}\"", and.trim()))?);
            }
            groups.push(group);
        }
        Ok(Some(Filter { groups }))
    }

    pub fn test(&self, v: f64) -> bool {
        self.groups.iter().any(|g| g.iter().all(|c| c.test(v)))
    }
}

/// `s` cut at each of `seps`, whichever comes first.
fn split_on(s: &str, seps: &[&str]) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut rest = s;
    'outer: while !rest.is_empty() {
        for sep in seps {
            if let Some(r) = rest.strip_prefix(sep) {
                out.push(String::new());
                rest = r;
                continue 'outer;
            }
        }
        let c = rest.chars().next().expect("not empty");
        out.last_mut().expect("never empty").push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn atom(s: &str) -> Option<Cond> {
    match s {
        "blank" | "empty" | "missing" => return Some(Cond::Blank),
        "!blank" | "not blank" | "!empty" | "not empty" => return Some(Cond::NotBlank),
        _ => {}
    }
    if let Some((a, b)) = s.split_once("..") {
        return Some(Cond::Range(number(a)?, number(b)?));
    }
    let (op, rest) = ["<=", ">=", "!=", "==", ">", "<", "="]
        .iter()
        .find_map(|op| s.strip_prefix(op).map(|r| (*op, r)))
        .unwrap_or(("=", s));
    let a = number(rest)?;
    Some(match op {
        ">" => Cond::Gt(a),
        ">=" => Cond::Ge(a),
        "<" => Cond::Lt(a),
        "<=" => Cond::Le(a),
        "!=" => Cond::Ne(a),
        _ => Cond::Eq(a),
    })
}

fn number(s: &str) -> Option<f64> {
    let s = s.trim();
    // only plain decimals: "inf" and "nan" parse as floats but aren't filter values
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'e' | b'+')))
        .then(|| s.parse().ok())
        .flatten()
}

/// A table column: the time, or a channel by index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Col {
    Time,
    Channel(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortKey {
    pub col: Col,
    pub descending: bool,
}

/// Rows `first..first + count` of a recording, filtered by columns (all must pass) and sorted by keys in order.
#[derive(Clone, Debug)]
pub struct Query {
    pub first: usize,
    pub count: usize,
    pub filters: Vec<(Col, Filter)>,
    pub sort: Vec<SortKey>,
}

/// The rows a query leaves, as offsets from [`Query::first`], in the query's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rows {
    pub ids: Vec<u32>,
}

/// Rows read at a time.
const BLOCK: usize = 1 << 16;

/// Runs `q`; `read(col, range)` gives a column's shown values (NaN where missing; the time in ns) for absolute
/// sample indices. `progress(done)` is called with 0..=1 about once a block and returns false to cancel (the
/// result is then `None`). Missing values sort last in both directions; ties keep the file's order.
pub fn select(
    q: &Query,
    read: &(dyn Fn(Col, Range<usize>) -> Vec<f64> + Sync),
    progress: &(dyn Fn(f64) -> bool + Sync),
) -> Option<Rows> {
    if q.count > u32::MAX as usize {
        return None;
    }
    let blocks = q.count.div_ceil(BLOCK).max(1);
    let steps = (blocks * (q.filters.len().max(1) + q.sort.len())) as f64;
    let mut done = 0usize;
    let step = |done: &mut usize| {
        *done += 1;
        progress(*done as f64 / steps)
    };
    // filter: each block's passing offsets, in order
    let mut ids: Vec<u32> = Vec::new();
    for b in 0..blocks {
        let lo = q.first + b * BLOCK;
        let hi = (lo + BLOCK).min(q.first + q.count);
        let mut keep: Vec<bool> = vec![true; hi - lo];
        for (col, f) in &q.filters {
            let v = read(*col, lo..hi);
            keep.iter_mut().zip(&v).for_each(|(k, x)| *k = *k && f.test(*x));
            if !step(&mut done) {
                return None;
            }
        }
        if q.filters.is_empty() && !step(&mut done) {
            return None;
        }
        ids.extend(keep.iter().enumerate().filter(|(_, k)| **k).map(|(i, _)| (lo - q.first + i) as u32));
    }
    if q.sort.is_empty() {
        return Some(Rows { ids });
    }
    // sort: the keys of the surviving rows, gathered block by block
    let mut keys: Vec<Vec<f64>> = Vec::new();
    for k in &q.sort {
        let mut col = Vec::with_capacity(ids.len());
        let mut at = 0;
        for b in 0..blocks {
            let lo = q.first + b * BLOCK;
            let hi = (lo + BLOCK).min(q.first + q.count);
            let end = at + ids[at..].partition_point(|&i| (i as usize) < hi - q.first);
            if end > at {
                let v = read(k.col, lo..hi);
                col.extend(ids[at..end].iter().map(|&i| v[i as usize + q.first - lo]));
                at = end;
            }
            if !step(&mut done) {
                return None;
            }
        }
        keys.push(col);
    }
    let cmp = |a: f64, b: f64, desc: bool| match (a.is_nan(), b.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        _ if desc => b.total_cmp(&a),
        _ => a.total_cmp(&b),
    };
    if let ([k], [col]) = (&q.sort[..], &keys[..]) {
        // one key: sort (key, row) pairs, which is much faster than chasing indices
        let mut pairs: Vec<(f64, u32)> = col.iter().copied().zip(ids.iter().copied()).collect();
        pairs.par_sort_by(|a, b| cmp(a.0, b.0, k.descending).then(a.1.cmp(&b.1)));
        return Some(Rows { ids: pairs.into_iter().map(|p| p.1).collect() });
    }
    let mut order: Vec<u32> = (0..ids.len() as u32).collect();
    order.par_sort_by(|&a, &b| {
        q.sort
            .iter()
            .zip(&keys)
            .map(|(k, c)| cmp(c[a as usize], c[b as usize], k.descending))
            .find(|o| o.is_ne())
            .unwrap_or(Ordering::Equal)
    });
    Some(Rows { ids: order.into_iter().map(|i| ids[i as usize]).collect() })
}

/// The runs of rows `ids` (any order) as `(first, last)` times: rows whose times are within `join` ns of the
/// previous match form one run, as event rules join theirs.
pub fn runs(times: &[Timestamp], join: Timestamp) -> Vec<(Timestamp, Timestamp)> {
    let mut out: Vec<(Timestamp, Timestamp)> = Vec::new();
    for &t in times {
        match out.last_mut() {
            Some(r) if t - r.1 <= join => r.1 = t,
            _ => out.push((t, t)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(s: &str) -> Filter {
        Filter::parse(s).unwrap().unwrap()
    }

    #[test]
    fn filter_syntax() {
        assert_eq!(Filter::parse("  ").unwrap(), None);
        assert!(f("> 1.5").test(2.0) && !f("> 1.5").test(1.5));
        assert!(f(">=1.5").test(1.5) && f("<= -1").test(-1.0) && f("< 0").test(-0.1));
        assert!(f("1..2").test(1.0) && f("1..2").test(2.0) && !f("1..2").test(2.1));
        assert!(f("2..1").test(1.5), "ends in either order");
        assert!(f("= 3").test(3.0) && f("3").test(3.0) && !f("3").test(4.0));
        assert!(f("!= 3").test(4.0) && !f("!= 3").test(f64::NAN));
        assert!(f("blank").test(f64::NAN) && !f("blank").test(0.0));
        assert!(f("not blank").test(0.0) && f("!empty").test(1.0));
    }

    #[test]
    fn filter_and_or() {
        let g = f("< 0 or > 10 and < 20");
        assert!(g.test(-1.0) && g.test(15.0) && !g.test(25.0) && !g.test(5.0));
        let h = f("> 1 && < 3 || blank");
        assert!(h.test(2.0) && h.test(f64::NAN) && !h.test(4.0));
        assert_eq!(f("> 1 AND < 3"), f("> 1 and < 3"), "case is ignored");
    }

    #[test]
    fn filter_errors_name_the_part() {
        assert_eq!(Filter::parse("> 1 and abc").unwrap_err(), "can't read \"abc\"");
        assert!(Filter::parse("inf").is_err() && Filter::parse(">").is_err() && Filter::parse("1..").is_err());
    }

    fn col(v: Vec<f64>) -> impl Fn(Col, Range<usize>) -> Vec<f64> + Sync {
        move |_, r| v[r].to_vec()
    }

    fn two(a: Vec<f64>, b: Vec<f64>) -> impl Fn(Col, Range<usize>) -> Vec<f64> + Sync {
        move |c, r| match c {
            Col::Channel(0) => a[r].to_vec(),
            Col::Channel(_) => b[r].to_vec(),
            Col::Time => r.map(|i| i as f64 * 1e6).collect(),
        }
    }

    fn run(q: &Query, read: &(dyn Fn(Col, Range<usize>) -> Vec<f64> + Sync)) -> Vec<u32> {
        select(q, read, &|_| true).unwrap().ids
    }

    fn q(count: usize) -> Query {
        Query { first: 0, count, filters: vec![], sort: vec![] }
    }

    #[test]
    fn unsorted_unfiltered_is_every_row_in_order() {
        assert_eq!(run(&q(4), &col(vec![0.0; 4])), vec![0, 1, 2, 3]);
        assert!(run(&q(0), &col(vec![])).is_empty());
    }

    #[test]
    fn filters_all_columns_must_pass() {
        let read = two(vec![1.0, 5.0, 9.0, 5.0], vec![0.0, 0.0, 1.0, 1.0]);
        let mut query = q(4);
        query.filters = vec![(Col::Channel(0), f("> 2")), (Col::Channel(1), f("1"))];
        assert_eq!(run(&query, &read), vec![2, 3]);
    }

    #[test]
    fn sort_orders_with_missing_last_and_ties_in_file_order() {
        let read = col(vec![3.0, f64::NAN, 1.0, 3.0, 2.0]);
        let mut query = q(5);
        query.sort = vec![SortKey { col: Col::Channel(0), descending: false }];
        assert_eq!(run(&query, &read), vec![2, 4, 0, 3, 1]);
        query.sort[0].descending = true;
        assert_eq!(run(&query, &read), vec![0, 3, 4, 2, 1], "missing stays last, ties keep file order");
    }

    #[test]
    fn multi_column_sort() {
        let read = two(vec![1.0, 2.0, 1.0, 2.0], vec![9.0, 1.0, 3.0, 5.0]);
        let mut query = q(4);
        query.sort = vec![
            SortKey { col: Col::Channel(0), descending: false },
            SortKey { col: Col::Channel(1), descending: true },
        ];
        assert_eq!(run(&query, &read), vec![0, 2, 3, 1]);
        query.sort = vec![SortKey { col: Col::Time, descending: true }];
        assert_eq!(run(&query, &read), vec![3, 2, 1, 0]);
    }

    #[test]
    fn a_span_in_the_middle_counts_offsets_from_its_first_row() {
        let read = col((0..10).map(|i| (10 - i) as f64).collect());
        let query = Query {
            first: 3,
            count: 4,
            filters: vec![(Col::Channel(0), f("< 7"))],
            sort: vec![SortKey { col: Col::Channel(0), descending: false }],
        };
        // rows 3..7 hold 7, 6, 5, 4; those under 7, ascending: 4, 5, 6 at offsets 3, 2, 1
        assert_eq!(run(&query, &read), vec![3, 2, 1]);
    }

    #[test]
    fn more_than_one_block_and_progress_and_cancel() {
        let n = BLOCK * 2 + 123;
        let v: Vec<f64> = (0..n).map(|i| ((i * 7919) % 1000) as f64).collect();
        let read = col(v.clone());
        let query = Query {
            first: 0,
            count: n,
            filters: vec![(Col::Channel(0), f("< 10"))],
            sort: vec![SortKey { col: Col::Channel(0), descending: true }],
        };
        let ids = run(&query, &read);
        let mut want: Vec<u32> = (0..n as u32).filter(|&i| v[i as usize] < 10.0).collect();
        want.sort_by(|&a, &b| v[b as usize].total_cmp(&v[a as usize]).then(a.cmp(&b)));
        assert_eq!(ids, want);
        let seen = std::sync::Mutex::new(Vec::new());
        select(&query, &read, &|d| {
            seen.lock().unwrap().push(d);
            true
        });
        let seen = seen.into_inner().unwrap();
        assert!(seen.windows(2).all(|w| w[0] <= w[1]) && (seen.last().unwrap() - 1.0).abs() < 1e-9, "{seen:?}");
        assert!(select(&query, &read, &|_| false).is_none(), "cancelled");
    }

    #[test]
    fn match_runs_join_under_the_gap() {
        let s = 1_000_000_000;
        let times = [0, s / 10, s / 5, s, s + s / 10, 5 * s];
        assert_eq!(runs(&times, s / 4), vec![(0, s / 5), (s, s + s / 10), (5 * s, 5 * s)]);
        assert!(runs(&[], s).is_empty());
    }
}
