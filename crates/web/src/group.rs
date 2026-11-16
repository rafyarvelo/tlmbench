//! Several files of an export on one timeline: a tab's slots, each with the map from its own clock to the shared one
//! (`tlmbench_core::timemap`). Channels are numbered slot after slot and slot `n` > 1 names its channels `name·n`, as
//! the app's groups do. Every query is answered per file by the same sparse-aware code a single file uses
//! (`ops.rs`): the span goes through the inverse of the file's map, and the times that come back through the map, so
//! changing an alignment rebuilds nothing and the integrity rule (a raw sample or an exact aggregate, never a guess)
//! holds across files.

use serde_json::{Value, json};
use tlmbench_core::timemap::TimeMap;

use crate::data::{Ds, State};

/// A tab's files: `(file id, map)` in slot order, the first being the primary.
pub struct Group {
    pub members: Vec<(u32, TimeMap)>,
}

/// One file of a tab as a query sees it.
pub struct Slot<'a> {
    pub ds: &'a Ds,
    pub map: TimeMap,
    /// The slot's first channel in the group's numbering.
    pub first: usize,
}

/// The slots of `id`: a group's files, or the one file with the identity map.
pub fn slots(st: &State, id: u32) -> Result<Vec<Slot<'_>>, String> {
    let file =
        |f: u32| st.files.get(f as usize).and_then(|x| x.as_ref()).ok_or_else(|| "that file is not open".to_string());
    match st.groups.get(&id) {
        Some(g) => {
            let mut first = 0;
            g.members
                .iter()
                .map(|(f, map)| {
                    let ds = file(*f)?;
                    let s = Slot { ds, map: map.clone(), first };
                    first += ds.info.channels.len();
                    Ok(s)
                })
                .collect()
        }
        None => Ok(vec![Slot { ds: file(id)?, map: TimeMap::identity(), first: 0 }]),
    }
}

/// The slot and the channel's number in its own file.
pub fn route(slots: &[Slot], c: usize) -> Option<(usize, usize)> {
    slots
        .iter()
        .enumerate()
        .find_map(|(i, s)| (c >= s.first && c < s.first + s.ds.info.channels.len()).then(|| (i, c - s.first)))
}

fn knots_in(v: &Value) -> Result<TimeMap, String> {
    let knots: Result<Vec<(i64, i64)>, String> = v
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|k| {
            let at = |i: usize| {
                k.get(i).and_then(Value::as_str).and_then(|s| s.parse::<i64>().ok()).ok_or("a knot is two times")
            };
            Ok((at(0)?, at(1)?))
        })
        .collect();
    TimeMap::new(knots?).map_err(|e| e.to_string())
}

/// The tab as the page's `group_set`, `group_state` and `group_map` answer: the merged channel list and every slot.
pub fn opened(st: &State, id: u32) -> Result<Value, String> {
    let sl = slots(st, id)?;
    let mut info = sl[0].ds.info.clone();
    info.channels.clear();
    for (i, s) in sl.iter().enumerate() {
        for c in &s.ds.info.channels {
            let mut c = c.clone();
            if i > 0 {
                c.name = format!("{}·{}", c.name, i + 1);
            }
            info.channels.push(c);
        }
    }
    // the shared timeline: the primary's start to the last file's end, a file that starts earlier extends the start
    let spans: Vec<Option<(i64, i64)>> =
        sl.iter().map(|s| s.ds.info.t_first.zip(s.ds.info.t_last).map(|(a, b)| s.map.span(a, b))).collect();
    let start = spans.iter().flatten().map(|s| s.0).min();
    let end = spans.iter().flatten().map(|s| s.1).max();
    info.t_first = start;
    info.t_last = end;
    let members =
        st.groups.get(&id).map(|g| g.members.iter().map(|(f, _)| *f).collect::<Vec<_>>()).unwrap_or_else(|| vec![id]);
    let slots: Vec<Value> = sl
        .iter()
        .zip(&members)
        .map(|(s, f)| {
            json!({
                "file": f, "first": s.first, "count": s.ds.info.channels.len(),
                "knots": s.map.knots.iter().map(|(a, b)| [a.to_string(), b.to_string()]).collect::<Vec<_>>(),
                "own": s.ds.info.t_first.zip(s.ds.info.t_last).map(|(a, b)| (a.to_string(), b.to_string())),
            })
        })
        .collect();
    Ok(json!({
        "id": id, "info": info,
        "range": start.zip(end).map(|(a, b)| (a.to_string(), b.to_string())),
        "status": { "state": "built" }, "slots": slots,
    }))
}

/// `group_set`: puts the files in `members` on one timeline (the first is the primary), as the group `group` or a new one.
pub fn set(st: &mut State, group: Option<u32>, members: &Value) -> Result<Value, String> {
    let list = members.as_array().filter(|m| !m.is_empty()).ok_or("A tab needs a file")?;
    let mut parts = Vec::new();
    for m in list {
        let f = m["file"].as_u64().ok_or("no file")? as u32;
        if st.files.get(f as usize).and_then(|x| x.as_ref()).is_none() {
            return Err("that file is not open".into());
        }
        parts.push((f, knots_in(&m["knots"])?));
    }
    // group ids start past the file ids an export can have
    let id = group.unwrap_or_else(|| (1000..).find(|i| !st.groups.contains_key(i)).expect("free id"));
    st.groups.insert(id, Group { members: parts });
    opened(st, id)
}

/// `group_map`: one slot's map replaced (a nudge, a method's result); nothing is rebuilt.
pub fn set_map(st: &mut State, id: u32, slot: usize, knots: &Value) -> Result<Value, String> {
    let map = knots_in(knots)?;
    let g = st.groups.get_mut(&id).ok_or("that tab has one file")?;
    g.members.get_mut(slot).ok_or("no such file in this tab")?.1 = map;
    opened(st, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlmbench_core::RateTimes;
    use tlmbench_core::recording::{ChannelInfo, TimeSource};

    pub fn file(n: usize, names: &[&str]) -> Ds {
        let mut d = Ds::new(
            "f",
            "export",
            None,
            n,
            TimeSource::Rate { hz: 1000.0 },
            crate::data::Tm::Rate(RateTimes::from_rate(0, 1000.0, n)),
            names
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
        for c in &mut d.cols {
            c.raw = crate::data::Raw::whole((0..n).map(|i| i as f64).collect());
        }
        for k in 0..d.cols.len() {
            d.build_pyramid(k);
        }
        d
    }

    #[test]
    fn channels_number_slot_after_slot_and_name_by_slot() {
        let mut st = State::default();
        st.files.push(Some(file(1000, &["a", "b"])));
        st.files.push(Some(file(500, &["a"])));
        let g = set(&mut st, None, &json!([{ "file": 0, "knots": [] }, { "file": 1, "knots": [["0", "250000000"]] }]))
            .unwrap();
        let names: Vec<&str> =
            g["info"]["channels"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["a", "b", "a·2"]);
        assert_eq!(g["slots"][1]["first"], 2);
        // the shared timeline is the primary's start to the last file's end (file 2 ends 0.25 s + 0.5 s in)
        assert_eq!(g["range"][0], "0");
        assert_eq!(g["range"][1], "999000000");
        let sl = slots(&st, g["id"].as_u64().unwrap() as u32).unwrap();
        assert_eq!(route(&sl, 2), Some((1, 0)));
        assert_eq!(route(&sl, 3), None);
        // a map that runs backwards is refused
        assert!(set(&mut st, None, &json!([{ "file": 0, "knots": [["5", "1"], ["4", "2"]] }])).is_err());
    }
}
