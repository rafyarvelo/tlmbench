//! Table sort and filter, as the app's `table_query`, `table_hit`, `table_marks`, `table_mask` and `table_unmask` answer
//! them (same arguments, same replies), over the rows the page holds: the sort and filter themselves are the core's
//! (`tlmbench_core::table`). The rows are the primary file's samples; in a tab of several files another file's column
//! is read as the table shows it, held at its last sample at or before each row's time. Times in replies are on the
//! tab's timeline. A query runs to its end within the call, so there is no progress to report and nothing to cancel.

use std::ops::Range;

use serde::Deserialize;
use serde_json::{Value, json};
use tlmbench_core::Recording;
use tlmbench_core::mask::Mask;
use tlmbench_core::table::{self, Col, Filter, Query, SortKey};

use crate::Reply;
use crate::data::State;
use crate::group::{Slot, route, slots};
use crate::ops::{MAX_ROWS, arg, time};

/// A sorted and filtered list of rows (see [`query`]).
pub struct RowQuery {
    /// Index of the span's first sample (the primary's); `ids` are offsets from it.
    first: usize,
    ids: Vec<u32>,
    /// The filter's matches as runs of time `(first, last)` ns on the tab's timeline, those closer than 0.25 s
    /// joined; empty without a filter.
    hits: Vec<(i64, i64)>,
}

/// One column of a table query: `t` (the time) or `c<channel>`, with the filter text (filters) or whether the sort
/// is descending (sorts).
#[derive(Deserialize)]
struct ColSpec {
    col: String,
    #[serde(default)]
    expr: String,
    #[serde(default)]
    desc: bool,
}

fn col_of(sl: &[Slot], s: &str) -> Result<Col, String> {
    let c = match s {
        "t" => Col::Time,
        _ => {
            s.strip_prefix('c').and_then(|n| n.parse().ok()).map(Col::Channel).ok_or_else(|| format!("column {s:?}"))?
        }
    };
    match c {
        Col::Channel(k) if route(sl, k).is_none() => Err(format!("no column {k}")),
        c => Ok(c),
    }
}

fn filters_of(sl: &[Slot], a: &Value) -> Result<Vec<(Col, Filter)>, String> {
    let mut fs = Vec::new();
    for f in arg::<Vec<ColSpec>>(a, "filters")? {
        if let Some(p) = Filter::parse(&f.expr)? {
            fs.push((col_of(sl, &f.col)?, p));
        }
    }
    Ok(fs)
}

/// Another file's sample shown at the tab's time `t`: its last at or before it (missing before its first, or where
/// the export doesn't hold it).
fn held_at(s: &Slot, local: usize, t: i64) -> f64 {
    s.ds.lower_bound(s.map.inverse(t).saturating_add(1))
        .checked_sub(1)
        .and_then(|i| s.ds.cols[local].raw.get(i))
        .unwrap_or(f64::NAN)
}

/// A column's shown values for the primary's samples `r` (NaN where missing or not held; the time in ns), as the
/// table shows them.
fn read(sl: &[Slot], col: Col, r: Range<usize>) -> Vec<f64> {
    let (d, pm) = (sl[0].ds, &sl[0].map);
    match col {
        Col::Time => d.read_times(r.start, r.end).into_iter().map(|t| pm.apply(t) as f64).collect(),
        Col::Channel(c) => match route(sl, c) {
            Some((0, l)) => {
                let raw = &d.cols[l].raw;
                match raw.slice(d.len) {
                    Some(v) => v[r].to_vec(),
                    None => r.map(|i| raw.get(i).unwrap_or(f64::NAN)).collect(),
                }
            }
            Some((si, l)) => {
                d.read_times(r.start, r.end).into_iter().map(|t| held_at(&sl[si], l, pm.apply(t))).collect()
            }
            None => vec![f64::NAN; r.len()],
        },
    }
}

/// The rows `first..first + count` that pass `filters`, in `sort` order.
fn select(sl: &[Slot], first: usize, count: usize, filters: Vec<(Col, Filter)>, sort: Vec<SortKey>) -> Vec<u32> {
    let q = Query { first, count, filters, sort };
    let read = |c: Col, r: Range<usize>| read(sl, c, r);
    table::select(&q, &read, &|_| true).map(|r| r.ids).unwrap_or_default()
}

fn done(matched: u64, total: u64, hits: u64) -> Reply {
    Reply::Json(json!({ "matched": matched, "total": total, "done": true, "hits": hits }))
}

/// `table_query`: sorts and filters the rows with times in `span` over their whole columns; `token` names the result,
/// which `rows` then pages through (`query`), and `dropToken` the table's last one, which goes.
pub fn query(st: &mut State, a: &Value) -> Result<Reply, String> {
    let token: u32 = arg(a, "token")?;
    if let Some(d) = a.get("dropToken").and_then(Value::as_u64) {
        st.queries.remove(&(d as u32));
    }
    let sl = slots(st, arg(a, "file")?)?;
    let span: Value = arg(a, "span")?;
    let (t0, t1) = (time(span["t0"].as_str().unwrap_or(""))?, time(span["t1"].as_str().unwrap_or(""))?);
    let filters = filters_of(&sl, a)?;
    let sort = arg::<Vec<ColSpec>>(a, "sort")?
        .iter()
        .map(|k| Ok(SortKey { col: col_of(&sl, &k.col)?, descending: k.desc }))
        .collect::<Result<Vec<_>, String>>()?;
    let (d, pm) = (sl[0].ds, &sl[0].map);
    let first = d.lower_bound(pm.inverse(t0));
    let count = d.lower_bound(pm.inverse(t1)).max(first) - first;
    let filtered = !filters.is_empty();
    let ids = select(&sl, first, count, filters, sort);
    let hits = if filtered {
        let mut asc = ids.clone();
        asc.sort_unstable();
        let times: Vec<i64> = asc.iter().map(|&i| pm.apply(d.time_at(first + i as usize))).collect();
        table::runs(&times, 250_000_000)
    } else {
        Vec::new()
    };
    let (matched, n) = (ids.len() as u64, hits.len() as u64);
    drop(sl);
    st.queries.insert(token, RowQuery { first, ids, hits });
    Ok(done(matched, count as u64, n))
}

/// `table_hit`: the `k`th match of a query's filter, for stepping with ▲ ▼: its run of time (ns as text) and the
/// place of its first row in the query's list; null when the query or the match is gone.
pub fn hit(st: &State, a: &Value) -> Result<Reply, String> {
    let sl = slots(st, arg(a, "file")?)?;
    let token: u32 = arg(a, "token")?;
    let k: u64 = arg(a, "k")?;
    let Some(q) = st.queries.get(&token) else { return Ok(Reply::Json(Value::Null)) };
    let Some(&(t0, t1)) = q.hits.get(k as usize) else { return Ok(Reply::Json(Value::Null)) };
    let (d, pm) = (sl[0].ds, &sl[0].map);
    let row = d.lower_bound(pm.inverse(t0)).saturating_sub(q.first) as u32;
    // the list is in row order unless sorted; either way the first row of the run is the first id at or after `row`
    let pos = if q.ids.is_sorted() {
        q.ids.partition_point(|&i| i < row)
    } else {
        let row = q.ids.iter().copied().filter(|&i| i >= row).min().unwrap_or(row);
        q.ids.iter().position(|&i| i == row).unwrap_or(0)
    };
    Ok(Reply::Json(json!({ "t0": t0.to_string(), "t1": t1.to_string(), "pos": pos as u64 })))
}

const MARKS_MAX: usize = 4000;

/// The runs (sorted, not overlapping) that touch `[t0, t1)`, with those closer than `min_gap` joined, and joined
/// further until at most `cap` are left.
fn runs_in(runs: &[(i64, i64)], t0: i64, t1: i64, min_gap: i64, cap: usize) -> Vec<(i64, i64)> {
    let a = runs.partition_point(|r| r.1 < t0);
    let b = a + runs[a..].partition_point(|r| r.0 < t1);
    let mut gap = min_gap.max(0);
    loop {
        let mut out: Vec<(i64, i64)> = Vec::new();
        for &(x, y) in &runs[a..b] {
            match out.last_mut() {
                Some(last) if x - last.1 <= gap => last.1 = y,
                _ => out.push((x, y)),
            }
        }
        if out.len() <= cap {
            return out;
        }
        gap = (gap * 2).max(1);
    }
}

/// `table_marks`: what the charts mark over `t0..t1` drawn `widthPx` wide: the matches of a query (`token`; none
/// without one) and the rows Apply to charts leaves out, as runs of time (ns as text), those under a pixel apart
/// joined, at most 4000 of each.
pub fn marks(st: &State, a: &Value) -> Result<Reply, String> {
    let sl = slots(st, arg(a, "file")?)?;
    let (t0, t1) = (time(&arg::<String>(a, "t0")?)?, time(&arg::<String>(a, "t1")?)?);
    let width = a.get("widthPx").and_then(Value::as_u64).unwrap_or(1).max(1);
    let px = ((t1 as i128 - t0 as i128) / i128::from(width)).clamp(1, i64::MAX as i128) as i64;
    let text = |v: Vec<(i64, i64)>| v.into_iter().map(|(a, b)| [a.to_string(), b.to_string()]).collect::<Vec<_>>();
    let hits = a
        .get("token")
        .and_then(Value::as_u64)
        .and_then(|t| st.queries.get(&(t as u32)))
        .map(|q| runs_in(&q.hits, t0, t1, px, MARKS_MAX))
        .unwrap_or_default();
    let (d, pm) = (sl[0].ds, &sl[0].map);
    let timed: Vec<(i64, i64)> =
        d.mask.runs().iter().map(|r| (pm.apply(d.time_at(r.start)), pm.apply(d.time_at(r.end - 1)))).collect();
    Ok(Reply::Json(json!({ "hits": text(hits), "excluded": text(runs_in(&timed, t0, t1, px, MARKS_MAX)) })))
}

/// The file a tab's rows are: the tab's primary, or the file itself.
fn primary(st: &State, id: u32) -> u32 {
    st.groups.get(&id).and_then(|g| g.members.first()).map_or(id, |m| m.0)
}

/// `table_mask` (Apply to charts): leaves the rows the filters don't pass out of every chart, statistic and value of
/// the file (`owner` names the table; a new call replaces its last), over the whole file. `table_unmask` takes it back.
pub fn mask(st: &mut State, a: &Value) -> Result<Reply, String> {
    let id: u32 = arg(a, "file")?;
    let owner: u32 = arg(a, "owner")?;
    let sl = slots(st, id)?;
    let filters = filters_of(&sl, a)?;
    let file = primary(st, id) as usize;
    if filters.is_empty() {
        drop(sl);
        if let Some(Some(d)) = st.files.get_mut(file) {
            d.set_mask(owner, None);
        }
        return Ok(Reply::Json(json!({ "matched": 0, "total": 0, "done": true, "hits": 0 })));
    }
    let n = sl[0].ds.len;
    let kept = select(&sl, 0, n, filters, Vec::new());
    let m = Mask::from_kept(0, n, &kept);
    let excluded = m.excluded() as u64;
    drop(sl);
    let d = st.files.get_mut(file).and_then(|f| f.as_mut()).ok_or("that file is not open")?;
    d.set_mask(owner, Some(m));
    Ok(done(n as u64 - excluded, n as u64, 0))
}

/// `table_unmask`: takes back what `table_mask` left out for `owner`.
pub fn unmask(st: &mut State, a: &Value) -> Result<Reply, String> {
    let id: u32 = arg(a, "file")?;
    let owner: u32 = arg(a, "owner")?;
    let file = primary(st, id) as usize;
    if let Some(Some(d)) = st.files.get_mut(file) {
        d.set_mask(owner, None);
    }
    Ok(Reply::Json(Value::Null))
}

/// `filter_check`: the problem with a filter's text, or nothing.
pub fn check(expr: &str) -> Option<String> {
    Filter::parse(expr).err()
}

/// A `rows` reply for rows `offset..offset + limit` of a query's list: as `rows` without one, flagged `1` at byte 28,
/// with each row's sample index (u64) between the times and the values.
pub fn query_rows(sl: &[Slot], channels: &[u32], q: &RowQuery, offset: u64, limit: u32) -> Vec<u8> {
    let (d, pm) = (sl[0].ds, &sl[0].map);
    let chans: Vec<(usize, usize)> = channels.iter().filter_map(|&c| route(sl, c as usize)).collect();
    let start = (offset as usize).min(q.ids.len());
    let end = start + (q.ids.len() - start).min(limit as usize).min(MAX_ROWS);
    let window = &q.ids[start..end];
    let k = window.len();
    let index: Vec<usize> = window.iter().map(|&i| q.first + i as usize).collect();
    let times: Vec<i64> = index.iter().map(|&i| pm.apply(d.time_at(i))).collect();
    let mut out = Vec::with_capacity(32 + 8 * k * (2 + chans.len()));
    out.extend_from_slice(b"TLMR");
    out.extend_from_slice(&(chans.len() as u32).to_le_bytes());
    out.extend_from_slice(&(q.first as u64).to_le_bytes());
    out.extend_from_slice(&(q.ids.len() as u64).to_le_bytes());
    out.extend_from_slice(&(k as u32).to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    times.iter().for_each(|t| out.extend_from_slice(&t.to_le_bytes()));
    index.iter().for_each(|&i| out.extend_from_slice(&(i as u64).to_le_bytes()));
    for &(si, local) in &chans {
        for (j, &t) in times.iter().enumerate() {
            // a sample the export doesn't hold is a blank cell
            let v =
                if si == 0 { d.cols[local].raw.get(index[j]).unwrap_or(f64::NAN) } else { held_at(&sl[si], local, t) };
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Ds, Raw, Tm};
    use crate::ops::dispatch;
    use tlmbench_core::RateTimes;
    use tlmbench_core::recording::{ChannelInfo, TimeSource};

    /// `n` samples at 1 kHz; channel `x` is `x(i)`, channel `on` is 1 for the first 500 ms of every second, else 0.
    fn file(n: usize) -> Ds {
        let mut d = Ds::new(
            "f",
            "export",
            None,
            n,
            TimeSource::Rate { hz: 1000.0 },
            Tm::Rate(RateTimes::from_rate(0, 1000.0, n)),
            ["x", "on"]
                .iter()
                .map(|n| ChannelInfo {
                    name: (*n).into(),
                    unit: None,
                    dtype: "f64",
                    category: None,
                    labels: Vec::new(),
                })
                .collect(),
        );
        d.cols[0].raw = Raw::whole((0..n).map(x).collect());
        d.cols[1].raw = Raw::whole((0..n).map(|i| if (i / 500) % 2 == 0 { 1.0 } else { 0.0 }).collect());
        for c in 0..2 {
            d.build_pyramid(c);
        }
        d
    }

    fn x(i: usize) -> f64 {
        ((i * 37) % 101) as f64
    }

    fn state(n: usize) -> State {
        let mut st = State::default();
        st.files.push(Some(file(n)));
        st
    }

    fn json(st: &mut State, cmd: &str, a: Value) -> Value {
        match dispatch(st, cmd, &a, &[]) {
            Ok(Reply::Json(v)) => v,
            Ok(Reply::Bytes(_)) => panic!("{cmd}: bytes"),
            Err(e) => panic!("{cmd}: {e}"),
        }
    }

    /// A `rows` reply: (first, total, flag, times, indices, values per channel).
    #[allow(clippy::type_complexity)]
    fn rows(st: &mut State, a: Value) -> Result<(u64, u64, u32, Vec<i64>, Vec<u64>, Vec<Vec<f64>>), String> {
        let Reply::Bytes(b) = dispatch(st, "rows", &a, &[])? else { panic!("rows: json") };
        let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let u64_at = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        assert_eq!(&b[..4], b"TLMR");
        let (nc, k, flag) = (u32_at(4) as usize, u32_at(24) as usize, u32_at(28));
        let mut o = 32;
        let times: Vec<i64> = (0..k).map(|j| u64_at(o + 8 * j) as i64).collect();
        o += 8 * k;
        let mut index = Vec::new();
        if flag == 1 {
            index = (0..k).map(|j| u64_at(o + 8 * j)).collect();
            o += 8 * k;
        }
        let vals = (0..nc).map(|c| (0..k).map(|j| f64::from_bits(u64_at(o + 8 * (c * k + j)))).collect()).collect();
        Ok((u64_at(8), u64_at(16), flag, times, index, vals))
    }

    fn q(st: &mut State, filters: Value, sort: Value, token: u32, drop: Option<u32>) -> Value {
        json(
            st,
            "table_query",
            json!({ "file": 0, "span": { "t0": "0", "t1": "99000000000" }, "filters": filters, "sort": sort, "token": token, "dropToken": drop }),
        )
    }

    fn page(st: &mut State, token: u32, offset: u64, limit: u32) -> (u64, u32, Vec<i64>, Vec<u64>, Vec<Vec<f64>>) {
        let a = json!({ "file": 0, "channels": [0, 1], "span": { "t0": "0", "t1": "1" }, "offset": offset, "limit": limit, "query": token });
        let (_, total, flag, t, i, v) = rows(st, a).unwrap();
        (total, flag, t, i, v)
    }

    #[test]
    fn sorts_ascending_and_descending_through_the_rows() {
        let mut st = state(3000);
        let r = q(&mut st, json!([]), json!([{ "col": "c0" }]), 1, None);
        assert_eq!(r, json!({ "matched": 3000, "total": 3000, "done": true, "hits": 0 }));
        let (total, flag, t, i, v) = page(&mut st, 1, 0, 3000);
        assert_eq!((total, flag, t.len()), (3000, 1, 3000));
        assert!(v[0].windows(2).all(|w| w[0] <= w[1]), "ascending");
        // each row is its own sample: the index, its time and both columns agree
        for j in 0..3000 {
            assert_eq!(v[0][j], x(i[j] as usize));
            assert_eq!(t[j], i[j] as i64 * 1_000_000);
        }
        // ties keep the file's order
        assert!(i.windows(2).all(|w| x(w[0] as usize) < x(w[1] as usize) || w[0] < w[1]));
        q(&mut st, json!([]), json!([{ "col": "c0", "desc": true }]), 2, Some(1));
        let (_, _, _, i, v) = page(&mut st, 2, 0, 3000);
        assert!(v[0].windows(2).all(|w| w[0] >= w[1]), "descending");
        assert_eq!(v[0][0], 100.0);
        assert_eq!(i[0] as usize, (0..3000).find(|&k| x(k) == 100.0).unwrap());
        // by time, descending: the last sample first
        q(&mut st, json!([]), json!([{ "col": "t", "desc": true }]), 3, Some(2));
        let (_, _, t, _, _) = page(&mut st, 3, 0, 2);
        assert_eq!(t, [2_999_000_000, 2_998_000_000]);
    }

    #[test]
    fn a_filter_leaves_matching_rows_and_its_matches_are_runs() {
        let mut st = state(3000);
        // the first half of each second: three runs 0.5 s apart (closer than 0.25 s would have joined)
        let r = q(&mut st, json!([{ "col": "c1", "expr": "> 0.5" }]), json!([]), 7, None);
        assert_eq!(r, json!({ "matched": 1500, "total": 3000, "done": true, "hits": 2 + 1 }));
        let (total, _, t, i, v) = page(&mut st, 7, 0, 10_000);
        assert_eq!((total, t.len()), (1500, 1500));
        assert!(v[1].iter().all(|&y| y == 1.0) && i.is_sorted());
        let hit = |st: &mut State, k: u64| json(st, "table_hit", json!({ "file": 0, "token": 7, "k": k }));
        assert_eq!(hit(&mut st, 0), json!({ "t0": "0", "t1": "499000000", "pos": 0 }));
        assert_eq!(hit(&mut st, 1), json!({ "t0": "1000000000", "t1": "1499000000", "pos": 500 }));
        assert_eq!(hit(&mut st, 3), Value::Null);
        // two filters must both pass; a sorted list's match is found at its place in the order
        let r = q(
            &mut st,
            json!([{ "col": "c1", "expr": "1" }, { "col": "c0", "expr": ">= 95" }]),
            json!([{ "col": "t", "desc": true }]),
            8,
            Some(7),
        );
        let want = (0..3000).filter(|&k| (k / 500) % 2 == 0 && x(k) >= 95.0).count() as u64;
        assert_eq!(r["matched"], want);
        let (_, _, t, _, v) = page(&mut st, 8, 0, 10_000);
        assert!(v[0].iter().all(|&y| y >= 95.0) && t.is_sorted_by(|a, b| a > b));
        let h = json(&mut st, "table_hit", json!({ "file": 0, "token": 8, "k": 0 }));
        let first = t.iter().copied().filter(|&ti| ti < 500_000_000).min().unwrap();
        assert_eq!(h["t0"], first.to_string());
        assert_eq!(t[h["pos"].as_u64().unwrap() as usize], first);
        // the charts' marks: the matches in view, none left out
        let m = json(
            &mut st,
            "table_marks",
            json!({ "file": 0, "token": 7, "t0": "0", "t1": "1200000000", "widthPx": 1000 }),
        );
        assert_eq!(m, json!({ "hits": [], "excluded": [] }), "token 7 was dropped");
        let m = json(
            &mut st,
            "table_marks",
            json!({ "file": 0, "token": 8, "t0": "0", "t1": "3000000000", "widthPx": 3000 }),
        );
        assert_eq!(m["hits"].as_array().unwrap().len(), 3, "{m}");
    }

    #[test]
    fn rows_page_through_a_query_and_a_dropped_token_is_gone() {
        let mut st = state(3000);
        q(&mut st, json!([{ "col": "c0", "expr": "< 50" }]), json!([{ "col": "c0" }]), 3, None);
        let (all_total, _, _, all, _) = page(&mut st, 3, 0, 10_000);
        let (total, flag, t, i, _) = page(&mut st, 3, 100, 25);
        assert_eq!((total, flag, t.len()), (all_total, 1, 25));
        assert_eq!(i, all[100..125]);
        // past the end: no rows; a limit is capped
        assert_eq!(page(&mut st, 3, all_total + 5, 10).2.len(), 0);
        // a new query with dropToken replaces the old: its rows, hits and marks are gone
        q(&mut st, json!([]), json!([{ "col": "c0" }]), 4, Some(3));
        let a = json!({ "file": 0, "channels": [0], "span": { "t0": "0", "t1": "1" }, "offset": 0, "limit": 10, "query": 3 });
        assert_eq!(rows(&mut st, a).unwrap_err(), "that query is gone");
        assert_eq!(json(&mut st, "table_hit", json!({ "file": 0, "token": 3, "k": 0 })), Value::Null);
        assert!(st.queries.contains_key(&4) && !st.queries.contains_key(&3));
        // cancelling is a no-op (a query has finished by then), and `rows` without a query is the span's rows
        assert_eq!(json(&mut st, "table_cancel", json!({ "token": 4 })), Value::Null);
        let a = json!({ "file": 0, "channels": [0], "span": { "t0": "0", "t1": "10000000" }, "offset": 0, "limit": 100, "query": null });
        let (first, total, flag, _, _, _) = rows(&mut st, a).unwrap();
        assert_eq!((first, total, flag), (0, 10, 0));
    }

    #[test]
    fn filter_check_and_bad_columns() {
        let mut st = state(10);
        assert_eq!(json(&mut st, "filter_check", json!({ "expr": "> 1.5" })), Value::Null);
        assert!(json(&mut st, "filter_check", json!({ "expr": "abc" })).is_string());
        let a = json!({ "file": 0, "span": { "t0": "0", "t1": "1" }, "filters": [], "sort": [{ "col": "c9" }], "token": 1 });
        assert_eq!(dispatch(&mut st, "table_query", &a, &[]).err().as_deref(), Some("no column 9"));
    }

    #[test]
    fn apply_to_charts_leaves_rows_out_of_values_stats_and_views_until_taken_back() {
        let mut st = state(3000);
        let m = json(
            &mut st,
            "table_mask",
            json!({ "file": 0, "owner": 5, "filters": [{ "col": "c1", "expr": "1" }], "token": 9 }),
        );
        assert_eq!(m, json!({ "matched": 1500, "total": 3000, "done": true, "hits": 0 }));
        // a value at a sample left out has none; one kept keeps its value
        let v = json(&mut st, "values_at", json!({ "file": 0, "channels": [0], "t": "700000000" }));
        assert_eq!(v[0]["value"], Value::Null);
        let v = json(&mut st, "values_at", json!({ "file": 0, "channels": [0], "t": "200000000" }));
        assert_eq!(v[0]["value"], x(200));
        let s =
            json(&mut st, "stats", json!({ "file": 0, "channels": [1], "span": { "t0": "0", "t1": "3000000000" } }));
        assert_eq!((s[0]["count"].as_u64(), s[0]["min"].as_f64()), (Some(1500), Some(1.0)), "{s}");
        let mk = json(
            &mut st,
            "table_marks",
            json!({ "file": 0, "token": null, "t0": "0", "t1": "3000000000", "widthPx": 3000 }),
        );
        assert_eq!(
            mk["excluded"],
            json!([["500000000", "999000000"], ["1500000000", "1999000000"], ["2500000000", "2999000000"]])
        );
        // the view breaks where the rows are left out: no point of the second half second
        let d = st.files[0].as_ref().unwrap();
        let vp = tlmbench_core::Viewport { t0: 0, t1: 3_000_000_000, width_px: 100, raw_max: 0 };
        let (tr, _, _) = crate::ops::channel_trace(d, 1, vp, 4.0, false);
        assert!(tr.t.iter().zip(&tr.y).all(|(&t, &y)| y.is_nan() || (t / 500_000_000) % 2 == 0), "{:?}", tr.y);
        // the table's rows still show every sample
        q(&mut st, json!([]), json!([{ "col": "c0" }]), 1, None);
        assert_eq!(page(&mut st, 1, 0, 1).0, 3000);
        // an empty filter takes it back, as Unmask does
        json(&mut st, "table_unmask", json!({ "file": 0, "owner": 5 }));
        let v = json(&mut st, "values_at", json!({ "file": 0, "channels": [0], "t": "700000000" }));
        assert_eq!(v[0]["value"], x(700));
        json(
            &mut st,
            "table_mask",
            json!({ "file": 0, "owner": 5, "filters": [{ "col": "c1", "expr": "1" }], "token": 9 }),
        );
        json(&mut st, "table_mask", json!({ "file": 0, "owner": 5, "filters": [], "token": 10 }));
        assert!(st.files[0].as_ref().unwrap().mask.is_empty());
    }

    #[test]
    fn a_tab_of_two_files_filters_by_the_other_files_column_held_at_each_row() {
        let mut st = state(3000);
        st.files.push(Some(file(3000)));
        // the second file runs 1 s late on the tab's timeline
        let g = crate::group::set(
            &mut st,
            None,
            &json!([{ "file": 0, "knots": [] }, { "file": 1, "knots": [["0", "1000000000"]] }]),
        )
        .unwrap();
        let id = g["id"].as_u64().unwrap();
        let r = json(
            &mut st,
            "table_query",
            json!({ "file": id, "span": { "t0": "0", "t1": "3000000000" }, "filters": [{ "col": "c3", "expr": "1" }], "sort": [], "token": 2 }),
        );
        // its `on` is 1 at the tab's 1..1.5 s and 2..2.5 s (and missing before 1 s)
        assert_eq!((r["matched"].as_u64(), r["hits"].as_u64()), (Some(1000), Some(2)), "{r}");
        let a = json!({ "file": id, "channels": [0, 3], "span": { "t0": "0", "t1": "1" }, "offset": 0, "limit": 1, "query": 2 });
        let (_, _, _, t, i, v) = rows(&mut st, a).unwrap();
        assert_eq!((t[0], i[0], v[1][0]), (1_000_000_000, 1000, 1.0));
    }
}
