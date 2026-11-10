//! Several recordings shown as one: a tab's files. Each keeps its own pyramid and sample index; a time map per file
//! (see [`tlmbench_core::timemap`]) takes it to the tab's shared timeline. Queries go through the inverse (the shared
//! span becomes the file's own span), and the times that come back go through the map, so changing an alignment
//! rebuilds nothing. Channels are numbered slot after slot: slot 1 (the primary) keeps its names, slot `n` reads
//! `accel_x·n`.

use std::sync::{Arc, RwLock};

use tlmbench_core::pyramid::Stats;
use tlmbench_core::timemap::TimeMap;
use tlmbench_core::{RecordingInfo, Timestamp, Viewport};

use crate::widgets::{Frame, WidgetOut, WidgetReq};
use crate::xy::{Xy, XyReq};
use crate::{Opened, Precision, Request, Status, View};

/// One file of a group.
#[derive(Debug)]
pub struct Slot {
    pub opened: Arc<Opened>,
    /// The file's clock to the shared timeline.
    map: RwLock<TimeMap>,
    /// The first channel's number in the group, and how many channels the file has.
    pub first: usize,
    pub count: usize,
}

impl Slot {
    pub fn map(&self) -> TimeMap {
        self.map.read().unwrap().clone()
    }
}

/// A tab's files on one timeline. Immutable in what it holds: adding, removing or promoting a file makes a new group
/// from the same opened files and maps.
#[derive(Debug)]
pub struct Group {
    slots: Vec<Slot>,
    info: RecordingInfo,
    ids: Vec<u32>,
}

/// A hover or cursor reading of one channel of a group.
#[derive(Clone, Debug)]
pub struct Reading {
    pub index: usize,
    /// On the shared timeline.
    pub t: Timestamp,
    /// In the file's own clock.
    pub own: Timestamp,
    pub value: f64,
}

impl Group {
    /// The files in slot order (the first is the primary) with their maps.
    pub fn new(files: Vec<(Arc<Opened>, TimeMap)>, ids: Vec<u32>) -> Self {
        assert!(!files.is_empty(), "a group has a file");
        let mut info = files[0].0.recording().info().clone();
        let mut slots = Vec::with_capacity(files.len());
        info.channels.clear();
        for (i, (opened, map)) in files.into_iter().enumerate() {
            let first = info.channels.len();
            let count = opened.recording().info().channels.len();
            for c in &opened.recording().info().channels {
                let mut c = c.clone();
                if i > 0 {
                    c.name = format!("{}·{}", c.name, i + 1);
                }
                info.channels.push(c);
            }
            slots.push(Slot { opened, map: RwLock::new(map), first, count });
        }
        let (a, b) = Self::bounds(&slots);
        info.t_first = a;
        info.t_last = b;
        Self { slots, info, ids }
    }

    /// The shared timeline's start and end: the primary's start to the last file's end (design: "primary file's
    /// start to the last file's end"); a file that starts earlier extends the start.
    fn bounds(slots: &[Slot]) -> (Option<Timestamp>, Option<Timestamp>) {
        let spans = slots.iter().filter_map(|s| {
            let i = s.opened.recording().info();
            let m = s.map.read().unwrap();
            Some((m.apply(i.t_first?), m.apply(i.t_last?)))
        });
        let (mut a, mut b) = (None::<i64>, None::<i64>);
        for (x, y) in spans {
            a = Some(a.map_or(x, |a| a.min(x)));
            b = Some(b.map_or(y, |b| b.max(y)));
        }
        (a, b)
    }

    /// All the channels, slot after slot, in the numbering the UI uses.
    pub fn info(&self) -> &RecordingInfo {
        &self.info
    }

    /// The caller's ids of the files, in slot order.
    pub fn file_ids(&self) -> &[u32] {
        &self.ids
    }

    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    /// The primary file.
    pub fn primary(&self) -> &Arc<Opened> {
        &self.slots[0].opened
    }

    /// The slot holding channel `c` and the channel's number in that file.
    pub fn route(&self, c: usize) -> Option<(usize, usize)> {
        let i = self.slots.partition_point(|s| s.first + s.count <= c);
        let s = self.slots.get(i)?;
        (c >= s.first).then_some((i, c - s.first))
    }

    /// Replaces file `slot`'s time map; the shared timeline's bounds follow in [`Group::bounds_now`].
    pub fn set_map(&self, slot: usize, map: TimeMap) {
        if let Some(s) = self.slots.get(slot) {
            *s.map.write().unwrap() = map;
        }
    }

    /// The timeline's current start and end (the maps can change after the group is made).
    pub fn bounds_now(&self) -> (Option<Timestamp>, Option<Timestamp>) {
        Self::bounds(&self.slots)
    }

    /// The group's files with their maps, to make the next group from.
    pub fn files(&self) -> Vec<(Arc<Opened>, TimeMap)> {
        self.slots.iter().map(|s| (s.opened.clone(), s.map())).collect()
    }

    /// Each request's channels reduced for its viewport, through the maps.
    pub fn view_batch(&self, requests: &[Request], precision: Precision) -> Vec<Vec<View>> {
        let maps: Vec<TimeMap> = self.slots.iter().map(Slot::map).collect();
        // per slot: the requests that touch it, as (request, the entry's channels' positions, sub-request)
        let mut per: Vec<Vec<(usize, Vec<usize>, Request)>> = vec![Vec::new(); self.slots.len()];
        for (ri, r) in requests.iter().enumerate() {
            for (si, slot) in self.slots.iter().enumerate() {
                let mut at = Vec::new();
                let (mut chans, mut gaps) = (Vec::new(), Vec::new());
                for (k, &c) in r.channels.iter().enumerate() {
                    if let Some((s, local)) = self.route(c)
                        && s == si
                    {
                        at.push(k);
                        chans.push(local);
                        gaps.push(r.gap_factors.get(k).copied().unwrap_or(tlmbench_core::gaps::DEFAULT_FACTOR));
                    }
                }
                if at.is_empty() {
                    continue;
                }
                let vp = Viewport { t0: maps[si].inverse(r.vp.t0), t1: maps[si].inverse(r.vp.t1), ..r.vp };
                let _ = slot;
                per[si].push((
                    ri,
                    at,
                    Request { channels: chans, gap_factors: gaps, vp, connect_gaps: r.connect_gaps },
                ));
            }
        }
        let mut out: Vec<Vec<Option<View>>> = requests.iter().map(|r| vec![None; r.channels.len()]).collect();
        for (si, jobs) in per.into_iter().enumerate() {
            if jobs.is_empty() {
                continue;
            }
            let subs: Vec<Request> = jobs.iter().map(|j| j.2.clone()).collect();
            let done = self.slots[si].opened.view_batch(&subs, precision);
            let (m, first) = (&maps[si], self.slots[si].first);
            for ((ri, at, _), views) in jobs.into_iter().zip(done) {
                for (k, mut v) in at.into_iter().zip(views) {
                    v.channel += first;
                    for t in &mut v.trace.t {
                        *t = m.apply(*t);
                    }
                    let (a, b) = (m.apply(0), m.apply(v.trace.bucket_ns));
                    v.trace.bucket_ns = (b - a).max(1);
                    v.built_until = v.built_until.map(|t| m.apply(t));
                    out[ri][k] = Some(v);
                }
            }
        }
        out.into_iter().map(|r| r.into_iter().flatten().collect()).collect()
    }

    /// Each channel's sample nearest the shared time `t`, in its own file (so files at different rates each give a
    /// real sample). `None` for a channel out of range or a file with no samples.
    pub fn readings(&self, channels: &[usize], t: Timestamp) -> Vec<Option<Reading>> {
        let mut out: Vec<Option<Reading>> = vec![None; channels.len()];
        for (si, slot) in self.slots.iter().enumerate() {
            let (mut at, mut local) = (Vec::new(), Vec::new());
            for (k, &c) in channels.iter().enumerate() {
                if let Some((s, l)) = self.route(c)
                    && s == si
                {
                    at.push(k);
                    local.push(l);
                }
            }
            if at.is_empty() {
                continue;
            }
            let m = slot.map();
            if let Some((i, own, values)) = slot.opened.values_at(&local, m.inverse(t)) {
                for (k, v) in at.into_iter().zip(values) {
                    out[k] = Some(Reading { index: i, t: m.apply(own), own, value: v });
                }
            }
        }
        out
    }

    /// Exact statistics of each channel's samples in the shared span `[t0, t1)`; None until every needed pyramid is
    /// built.
    pub fn stats(&self, channels: &[usize], t0: Timestamp, t1: Timestamp) -> Option<Vec<Stats>> {
        let mut out: Vec<Option<Stats>> = vec![None; channels.len()];
        for (si, slot) in self.slots.iter().enumerate() {
            let (mut at, mut local) = (Vec::new(), Vec::new());
            for (k, &c) in channels.iter().enumerate() {
                if let Some((s, l)) = self.route(c)
                    && s == si
                {
                    at.push(k);
                    local.push(l);
                }
            }
            if at.is_empty() {
                continue;
            }
            let m = slot.map();
            let got = slot.opened.stats(&local, m.inverse(t0), m.inverse(t1))?;
            for (k, s) in at.into_iter().zip(got) {
                out[k] = Some(s);
            }
        }
        out.into_iter().collect()
    }

    /// Every widget's answer at the shared time `t`: each file answers its own widgets (through its map), the frame is
    /// the first file's sample.
    pub fn widgets_frame(&self, t: Timestamp, view: (Timestamp, Timestamp), reqs: &[WidgetReq]) -> Option<Frame> {
        let mut out: Vec<WidgetOut> = vec![WidgetOut::default(); reqs.len()];
        let mut head: Option<(usize, Timestamp)> = None;
        for (si, slot) in self.slots.iter().enumerate() {
            let (mut at, mut sub) = (Vec::new(), Vec::new());
            for (k, r) in reqs.iter().enumerate() {
                if let Some((s, local)) = self.route(r.channel)
                    && s == si
                {
                    at.push(k);
                    sub.push(WidgetReq { channel: local, ..*r });
                }
            }
            if at.is_empty() {
                continue;
            }
            let m = slot.map();
            let f = slot.opened.widgets_frame(m.inverse(t), (m.inverse(view.0), m.inverse(view.1)), &sub)?;
            head.get_or_insert((f.index, m.apply(f.t)));
            for (k, mut w) in at.into_iter().zip(f.widgets) {
                w.run_start = w.run_start.map(|t| m.apply(t));
                out[k] = w;
            }
        }
        head.map(|(index, t)| Frame { index, t, widgets: out })
    }

    /// An XY plot of channels of one file (pairing is by sample, so they share their times); `None` when the
    /// channels are in different files.
    pub fn xy_points(&self, q: &XyReq) -> Option<Xy> {
        let (sx, lx) = self.route(q.x)?;
        let mut ys = Vec::with_capacity(q.ys.len());
        for &y in &q.ys {
            let (s, l) = self.route(y)?;
            if s != sx {
                return None;
            }
            ys.push(l);
        }
        let slot = &self.slots[sx];
        let m = slot.map();
        let sub = XyReq {
            x: lx,
            ys,
            t0: m.inverse(q.t0),
            t1: m.inverse(q.t1),
            rect: q.rect,
            grid: q.grid,
            max_points: q.max_points,
        };
        let mut r = slot.opened.xy_points(&sub);
        for s in &mut r.series {
            s.channel += slot.first;
            s.t.iter_mut().for_each(|t| *t = m.apply(*t));
        }
        Some(r)
    }

    /// A column of another file read at the rows' shared times: each row shows the file's last sample at or before
    /// its time (NaN before the file's first), so a slower channel is held between its samples. Rows may be in any order.
    pub fn held_column(&self, channel: usize, times: &[Timestamp]) -> Vec<f64> {
        let nan = vec![f64::NAN; times.len()];
        let Some((si, local)) = self.route(channel) else { return nan };
        let slot = &self.slots[si];
        let (m, rec) = (slot.map(), slot.opened.recording());
        let (scale, offset) = rec.scale_offset(local);
        let at: Vec<Option<usize>> =
            times.iter().map(|&t| rec.lower_bound(m.inverse(t).saturating_add(1)).checked_sub(1)).collect();
        let (Some(lo), Some(hi)) = (at.iter().flatten().min().copied(), at.iter().flatten().max().copied()) else {
            return nan;
        };
        if hi - lo < (1 << 20) {
            let v = rec.read(local, lo, hi + 1).to_f64(scale, offset);
            at.iter().map(|i| i.map_or(f64::NAN, |i| v[i - lo])).collect()
        } else {
            at.iter().map(|i| i.map_or(f64::NAN, |i| rec.read(local, i, i + 1).to_f64(scale, offset)[0])).collect()
        }
    }

    /// The channels of every file but the primary, as an expression over the primary's samples may name them
    /// (`Close·2`): each read through the file's map.
    pub fn foreign(&self) -> Vec<crate::Foreign> {
        self.slots
            .iter()
            .enumerate()
            .skip(1)
            .flat_map(|(i, s)| {
                let map = s.map();
                s.opened.recording().info().channels.iter().enumerate().map(move |(k, c)| crate::Foreign {
                    name: format!("{}·{}", c.name, i + 1),
                    opened: s.opened.clone(),
                    channel: k,
                    map: map.clone(),
                })
            })
            .collect()
    }

    /// Building: the files' progress together (each file's samples weigh the same as its share of the total).
    pub fn status(&self) -> Status {
        let (mut done, mut total, mut missing) = (0u64, 0u64, false);
        for s in &self.slots {
            let n = s.opened.recording().info().samples as u64;
            total += n;
            match s.opened.status() {
                Status::Built => done += n,
                Status::Building { done: d, .. } => done += d,
                Status::Missing => missing = true,
            }
        }
        if done >= total && !missing {
            Status::Built
        } else if done == 0 && missing {
            Status::Missing
        } else {
            Status::Building { done, total }
        }
    }

    /// Builds every file's pyramid, calling `progress(done, total)` over the files together.
    pub fn build(&self, progress: &mut dyn FnMut(u64, u64)) -> crate::Result<()> {
        let total: u64 = self.slots.iter().map(|s| s.opened.recording().info().samples as u64).sum();
        let mut before = 0u64;
        for s in &self.slots {
            let n = s.opened.recording().info().samples as u64;
            s.opened.build(&mut |d, t| progress(before + d.min(t) * n / t.max(1), total))?;
            before += n;
        }
        Ok(())
    }

    pub fn cancel(&self) {
        for s in &self.slots {
            s.opened.cancel();
        }
    }
}
