// The Gauges view's logic apart from drawing: which widget a channel gets, pairs and groups found by name, ranges
// and limit zones (design: Gauges and widgets). The core answers each frame (`widgets_frame`); nothing here reads data.

import { type Limits } from './display.svelte';
import type { WidgetCfg } from './stack.svelte';

/** A channel as the layout sees it. */
export interface GChan {
  ch: number;
  name: string;
  unit: string;
  /** The channel has value labels (an enum). */
  labelled: number;
}

export interface Layout {
  type: string;
  /** The second channel of a commanded-vs-actual or XY pair. */
  ref?: GChan;
  /** The members of a bar group. */
  members?: GChan[];
}

export const MULTI: Record<string, boolean> = { bullet: true, group: true, xy: true };
/** Widgets that mark the lowest and highest value in view. */
export const HOLD_TYPES: Record<string, boolean> = { arc: true, bar: true, tank: true, group: true, bullet: true };

export const TYPES: { type: string; name: string; hint: string }[] = [
  { type: 'arc', name: 'Gauge', hint: 'Semicircle dial with a needle, like an rpm gauge' },
  { type: 'ring', name: 'Ring', hint: 'Circular progress, 0–100% of the range' },
  { type: 'bar', name: 'Bar', hint: 'Horizontal bar with limit marks' },
  { type: 'tank', name: 'Tank', hint: 'Vertical level, like a thermometer or fuel tank' },
  { type: 'spark', name: 'Sparkline', hint: 'Value with its trace over the time view' },
  { type: 'state', name: 'State light', hint: 'A light coloured by limits, with the value label' },
  { type: 'compass', name: 'Compass', hint: 'Heading in degrees on a dial' },
  { type: 'horizon', name: 'Horizon', hint: 'Artificial horizon from roll and pitch' },
  { type: 'number', name: 'Number', hint: 'Big value with its trend over the last second' },
  { type: 'flags', name: 'Flags', hint: 'One light per bit of a status word' },
  { type: 'timeline', name: 'State timeline', hint: 'The current state, with a strip of states across the view' },
  { type: 'bullet', name: 'Commanded vs actual', hint: 'Actual as a bar, the command or target as a tick, and the difference' },
  { type: 'group', name: 'Bar group', hint: 'Similar channels side by side on one scale, so the odd one out stands out' },
  { type: 'xy', name: 'XY dot', hint: 'Two channels as a dot with a short trail: g-g diagram, stick position, or GPS track' },
];
export const typeName = (t: string) => TYPES.find((x) => x.type === t)?.name ?? t;

/** The widget a channel gets on its own, from its unit and name. */
export function autoWidget(c: GChan): string {
  const n = c.name.toLowerCase();
  const u = c.unit.toLowerCase();
  if (u === 'bits') return 'flags';
  if (u === 'flag') return 'state';
  if (u === 'enum' || c.labelled) return c.labelled > 2 || u === 'enum' ? 'timeline' : 'state';
  if (/yaw|heading|course|bearing/.test(n)) return 'compass';
  if (/\broll\b|^roll/.test(n) && u === 'deg') return 'horizon';
  if (u === 'rpm' || /rpm/.test(n)) return 'arc';
  if (u === '%') return 'ring';
  if (u === '°c' || u === 'c' || /temp/.test(n)) return 'tank';
  if (u === 'v') return 'arc';
  if (u === 'a' || u === 'w') return 'bar';
  return 'spark';
}

// Pairs and groups, found from names within the pane: a channel and its command or target (motor_rpm +
// motor_rpm_target, *_cmd, *_set, *_sp), an x/y pair or latitude/longitude, and three or more channels that differ
// only by a number (motor_2_rpm, cell_4_v). Your choice in the widget menu overrides each.
const TARGET_SUFFIX = ['_target', '_cmd', '_command', '_set', '_setpoint', '_sp', '_req', '_demand'];
export const groupKey = (n: string) => n.toLowerCase().replace(/_?\d+/g, '');

export function targetOf(c: GChan, chans: GChan[]): GChan | null {
  const n = c.name.toLowerCase();
  const b = n.replace(/_(actual|act|meas|fb|feedback)$/, '');
  return chans.find((o) => o !== c && TARGET_SUFFIX.some((s) => { const m = o.name.toLowerCase(); return m === n + s || (b !== n && m === b + s); })) ?? null;
}

export function xyPartner(c: GChan, chans: GChan[]): GChan | null {
  const n = c.name.toLowerCase();
  const m = n.match(/^(.*[_.])x$/);
  if (m) return chans.find((o) => o.name.toLowerCase() === m[1] + 'y') ?? null;
  if (/lat(itude)?$/.test(n)) {
    const want = n.replace(/lat(itude)?$/, (s) => (s.length > 3 ? 'longitude' : 'lon'));
    return chans.find((o) => o.name.toLowerCase() === want) ?? null;
  }
  return null;
}

/** What each channel of a pane is drawn as, by channel number; a channel shown inside another's widget is absent. */
export function layoutOf(chans: GChan[], widgets: Record<string, WidgetCfg> = {}): Map<number, Layout> {
  const cfgOf = (c: GChan) => widgets[c.name] ?? {};
  const used = new Set<GChan>();
  const out = new Map<number, Layout>();
  const secondary = new Set(chans.flatMap((c) => [targetOf(c, chans), xyPartner(c, chans)]).filter((x): x is GChan => !!x));
  const prim = chans.filter((c) => !secondary.has(c));
  const order = [...prim.filter((c) => MULTI[cfgOf(c).type ?? '']), ...prim.filter((c) => !MULTI[cfgOf(c).type ?? '']), ...chans.filter((c) => secondary.has(c))];
  for (const c of order) {
    if (used.has(c)) continue;
    const ex = cfgOf(c).type;
    const free = (o: GChan | null) => !!o && !used.has(o) && !cfgOf(o).type;
    const tg = targetOf(c, chans);
    if (tg && !used.has(tg) && (ex === 'bullet' || (!ex && free(tg)))) {
      used.add(c).add(tg);
      out.set(c.ch, { type: 'bullet', ref: tg });
      continue;
    }
    const xy = xyPartner(c, chans);
    if (xy && !used.has(xy) && (ex === 'xy' || (!ex && free(xy)))) {
      used.add(c).add(xy);
      out.set(c.ch, { type: 'xy', ref: xy });
      continue;
    }
    const sib = chans.filter((o) => !used.has(o) && !secondary.has(o) && groupKey(o.name) === groupKey(c.name) && o.unit === c.unit && (o === c || (!cfgOf(o).type && !targetOf(o, chans) && !xyPartner(o, chans))));
    if (ex === 'group' ? sib.length >= 2 : !ex && !secondary.has(c) && sib.length >= 3) {
      sib.forEach((o) => used.add(o));
      out.set(c.ch, { type: 'group', members: sib });
      continue;
    }
    used.add(c);
    out.set(c.ch, { type: ex && !MULTI[ex] ? ex : autoWidget(c) });
  }
  return out;
}

/** Short names for a pair: what differs (motor_rpm + motor_rpm_target → actual, target; accel_x + accel_y → x, y). */
export function partRoles(a: string, b: string): [string, string] {
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  if (i < a.length) while (i > 0 && !/[_.\s]/.test(a[i - 1])) i--;
  const ra = a.slice(i).replace(/^[_.\s]+/, '');
  const rb = b.slice(i).replace(/^[_.\s]+/, '');
  return [ra || 'actual', rb || b];
}

/** The label that tells group members apart: the number that differs (motor_rpm, motor_2_rpm → 1, 2). */
export function memberLabels(names: string[]): string[] {
  const ds = names.map((n) => n.match(/\d+/g) ?? []);
  const len = Math.max(0, ...ds.map((d) => d.length));
  for (let i = 0; i < len; i++) if (new Set(ds.map((d) => d[i])).size > 1) return ds.map((d) => (d[i] != null ? String(+d[i]) : '1'));
  return names;
}

export const LVL_COLOR: Record<string, string> = { warn: 'var(--warn)', alarm: 'var(--danger)', ok: 'var(--good)' };

/** The limit zones along a range, as [from, to, colour] in value units. */
export function zonesOf(lim: Limits, lo: number, hi: number): [number, number, string][] {
  const z: [number, number, string][] = [];
  if (lim.wh != null) z.push([lim.wh, lim.ah ?? hi, LVL_COLOR.warn]);
  if (lim.ah != null) z.push([lim.ah, hi, LVL_COLOR.alarm]);
  if (lim.wl != null) z.push([lim.al ?? lo, lim.wl, LVL_COLOR.warn]);
  if (lim.al != null) z.push([lo, lim.al, LVL_COLOR.alarm]);
  return z.map(([a, b, col]): [number, number, string] => [Math.max(lo, Math.min(hi, a)), Math.max(lo, Math.min(hi, b)), col]).filter(([a, b]) => b > a);
}

export function niceStep(span: number, n: number): number {
  const raw = span / Math.max(1, n);
  const p = 10 ** Math.floor(Math.log10(raw));
  const f = raw / p;
  return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10) * p;
}

/** The range a widget spans: yours, else the recording's min–max widened to include the limits and rounded to tidy
 * steps (gauges of positive things start at zero). */
export function rangeOf(data: [number, number] | null, lim: Limits, cfg: WidgetCfg): [number, number] {
  if (cfg.min != null && cfg.max != null && Number.isFinite(cfg.min) && Number.isFinite(cfg.max) && cfg.max > cfg.min) return [cfg.min, cfg.max];
  let [lo, hi] = data ?? [Infinity, -Infinity];
  for (const x of [lim.al, lim.wl, lim.wh, lim.ah]) if (x != null) [lo, hi] = [Math.min(lo, x), Math.max(hi, x)];
  if (!Number.isFinite(lo)) return [0, 1];
  if (lo >= 0 && lo < hi * 0.25) lo = 0;
  const span = hi - lo || Math.abs(hi) || 1;
  const step = niceStep(span, 4);
  return [lo === 0 ? 0 : Math.floor(lo / step) * step, Math.ceil(hi / step) * step];
}

/** A value for a widget: a few digits, thousands grouped from 10,000, a dash for none. */
export const gNum = (v: number | null | undefined, dec?: number): string =>
  v == null || !Number.isFinite(v) ? '—' : Math.abs(v) >= 10000 ? Math.round(v).toLocaleString() : v.toFixed(Math.min(dec ?? 2, 3));

export const polar = (cx: number, cy: number, r: number, deg: number): [number, number] => {
  const a = ((deg - 90) * Math.PI) / 180;
  return [cx + r * Math.cos(a), cy + r * Math.sin(a)];
};
export function arcPath(cx: number, cy: number, r: number, a0: number, a1: number): string {
  const [x0, y0] = polar(cx, cy, r, a0);
  const [x1, y1] = polar(cx, cy, r, a1);
  return `M${x0.toFixed(2)} ${y0.toFixed(2)}A${r} ${r} 0 ${Math.abs(a1 - a0) > 180 ? 1 : 0} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
}

/** One widget's reading from a frame. */
export interface Reading {
  value: number | null;
  min: number | null;
  max: number | null;
  /** The time the current run of equal values began (ns), for state widgets. */
  runStart: bigint | null;
  /** The channel's trace over the view, as `[fraction, value]` points (null breaks the line). */
  trace: [number, number | null][];
}

/** Everything a widget needs about one channel at the reading time. */
export interface Ctx {
  ch: number;
  name: string;
  /** What tells this channel apart inside a pair or group. */
  short: string;
  unit: string;
  v: number;
  ok: boolean;
  lo: number;
  hi: number;
  /** Where the value stands in the range, 0–1. */
  p: number;
  f: (v: number) => number;
  lvl: '' | 'ok' | 'warn' | 'alarm';
  col: string;
  zones: [number, number, string][];
  /** The value as text (or its label), a dash when missing. */
  txt: string;
  label: string | null;
  dec: number | undefined;
  /** The lowest and highest value since the start of the view, when marks are on and known. */
  hold: [number, number] | null;
  cfg: WidgetCfg;
}

/** What the widgets of a pane share beyond a channel's own context. */
export interface Extra {
  /** Horizon: the pitch beside roll. */
  pitch: number | null;
  /** Number: the change over the last second. */
  trend: number;
  /** Sparkline and state timeline: the trace over the view, and where the reading time falls in it (0–1). */
  trace: [number, number | null][];
  tFrac: number | null;
  /** State timeline: how long the state has lasted, in seconds. */
  since: number | null;
  /** XY dot: the view's cloud, the trail, the dot, and whether the pair is longitude/latitude. */
  xy: { view: [number, number][]; trail: [number, number][]; now: [number, number] | null; geo: boolean };
  /** The ends of the view, as text, for the timeline. */
  t0: string;
  t1: string;
  uid: string;
}

const PALETTE = 8;
/** The colour of state number `v`: one of the trace colours, round and round. */
export const stateColor = (v: number): string => `var(--trace-${(((Math.round(v) % PALETTE) + PALETTE) % PALETTE) + 1})`;

/** One light per bit of a status word, named when the channel names them. */
export function flagBits(hi: number): { n: string; bad?: boolean }[] {
  const n = Math.min(16, Math.max(1, Math.ceil(Math.log2((hi || 1) + 1))));
  return Array.from({ length: n }, (_, k) => ({ n: `bit ${k}` }));
}

/** The runs of equal state across a trace, as fractions of the view. */
export function stateSegs(trace: [number, number | null][], until: number): { a: number; b: number; v: number }[] {
  const out: { a: number; b: number; v: number }[] = [];
  for (let k = 0; k < trace.length; k++) {
    const [x, y] = trace[k];
    if (y == null || x > until) continue;
    const v = Math.round(y);
    const next = Math.min(until, trace[k + 1]?.[0] ?? until);
    const last = out[out.length - 1];
    if (last && last.v === v && Math.abs(last.b - x) < 0.02) last.b = Math.max(last.b, next);
    else if (next > x) out.push({ a: x, b: next, v });
  }
  return out;
}
