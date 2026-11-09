// The pane stack: panes in rows, a row's panes side by side in columns, a column's panes stacked (the prototype's
// model). Each pane lists its channels; `beside` puts it right of the pane before, `under` below it in the same
// column. app.ts draws into each pane's plot; PaneStack.svelte renders the panes, and these edits change them.

import { PAD, value } from './axes';
import { setSetting, settings } from './settings.svelte';
import { ui } from './ui.svelte';

/** How a pane shows its channels: the chart (the default, left out of saved layouts), a table of the samples in
 * view, the chart with its table below, or value tiles. */
export type PaneMode = 'chart' | 'table' | 'both' | 'values' | 'spectrum' | 'checks' | 'gauges' | 'xy';

/** One of a pane's value axes (design: Pane axes); what's left out is the default (fit the view, auto decimals). */
export interface AxisSettings {
  /** Fit ignoring outliers (the 0.1st–99.9th percentile; spikes are clipped and marked), fit including zero, or fixed. */
  range?: 'robust' | 'zero' | 'fixed';
  min?: number;
  max?: number;
  dec?: number;
  label?: string;
  log?: boolean;
}

/** A pane's axes: u0 for its first unit (left), u1 for its second (right). */
export type AxisKey = 'u0' | 'u1';

export interface Pane {
  channels: number[];
  mode?: PaneMode;
  axis?: Partial<Record<AxisKey, AxisSettings>>;
  /** Table column widths by column id ('n', 't', 'c<channel>'), where resized. */
  widths?: Record<string, number>;
  /** The chart's plot height in px, where dragged (a row's panes share it). */
  h?: number;
  /** The table's height under a chart ('both'), where dragged. */
  tableH?: number;
  /** Table sort keys in order, by column id ('t', 'c<channel>'; the sample column can't sort). */
  sort?: { col: string; desc: boolean }[];
  /** Table filters by column id: the text typed (`> 1.5`, `1..2`, `blank`, joined with and / or). */
  filters?: Record<string, string>;
  /** Where a table's filter looks: `scope` 'file' for every row (else the rows in view), and `apply` to leave the rows it
   *  filters out from this file's charts, statistics and values (always over the whole file). */
  tfilt?: { scope?: 'file'; apply?: boolean };
  /** Channels whose columns the table leaves out (they stay in the pane and its chart). */
  hideCols?: number[];
  /** Event markers on this pane's chart (false: hidden here; the top bar's E and the categories decide underneath). */
  events?: boolean;
  /** Gauges: each channel's widget choices by channel name (type, range, size, min/max marks, ...). */
  widgets?: Record<string, WidgetCfg>;
  /** XY: the X channel (the pane's first channel when left out), whether the axes keep equal scale (on for a
   *  longitude/latitude pair), and the local zoom as `[x0, x1, y0, y1]`. */
  x?: number;
  xyEqual?: boolean;
  xyView?: [number, number, number, number];
  beside?: boolean;
  under?: boolean;
}

/** What a person chose for one channel's widget; what's left out is automatic. */
export interface WidgetCfg {
  /** A built-in type (`arc`, `ring`, ...) or the id of a widget of your own. */
  type?: string;
  min?: number;
  max?: number;
  size?: 'S' | 'M' | 'L' | '2x2';
  /** Min and max in view marks (on unless false). */
  hold?: boolean;
  /** Commanded vs actual: the difference that is fine, in percent. */
  tol?: number;
  /** XY dot: the trail's length in seconds. */
  trail?: number;
}

/** Columns a row holds at most. */
export const MAX_PER_ROW = 3;

export const stack = $state({
  panes: [] as Pane[],
  sel: 0,
  /** Each channel's value at the pointer, shown in its chips. */
  values: {} as Record<number, string>,
  /** Each channel's value when the pointer is off the charts: at cursor A, else at the end of the view. */
  resting: {} as Record<number, string>,
  /** Where each channel's shown value stands against its limits (tiles take their colour from it). */
  levels: {} as Record<number, string>,
  restingLevels: {} as Record<number, string>,
  /** Panes drawn from summaries for now (labelled "coarse" until the exact view comes). */
  coarse: [] as boolean[],
});

/** Decimals each channel's values show, by `file:channel`, learned from the first axis range that drew it. */
export const decs: Record<string, number> = $state({});

/** Learns channel `ch`'s decimals from a range it was drawn over: four digits at its largest magnitude, 0 to 6 decimals. */
export function learnDecimals(file: number, ch: number, lo: number, hi: number): void {
  const k = `${file}:${ch}`;
  const m = Math.max(Math.abs(lo), Math.abs(hi));
  if (k in decs || !Number.isFinite(m) || m === 0) return;
  decs[k] = Math.max(0, Math.min(6, 3 - Math.floor(Math.log10(m))));
}

/** A channel's value in its decimals (about six significant digits until they're known). */
export function fmtCh(file: number | null, ch: number, v: number | null | undefined): string {
  const name = ui.channels[ch]?.name ?? '';
  const label = v == null ? undefined : settings.valueMaps[name]?.[String(v)];
  if (label != null) return label;
  const d = settings.chdisp[name]?.dec ?? (file == null ? undefined : decs[`${file}:${ch}`]);
  if (d == null || v == null || !Number.isFinite(v)) return value(v);
  return v.toFixed(d);
}

/** Called after every edit (app.ts saves the layout and asks for a view). */
let changed = () => {};
export function onStackChange(f: () => void): void {
  changed = f;
}

/** Rows of pane indexes: a pane starts a new row unless it is beside or under the one before. */
export function rows(): number[][] {
  const out: number[][] = [];
  stack.panes.forEach((p, i) => {
    if ((!p.beside && !p.under) || !out.length) out.push([i]);
    else out[out.length - 1].push(i);
  });
  return out;
}

/** A row's columns: each a run of panes, the first one beside (or starting the row), the rest under it. */
export function columns(row: number[]): number[][] {
  const out: number[][] = [];
  for (const i of row) {
    if (stack.panes[i].under && out.length) out[out.length - 1].push(i);
    else out.push([i]);
  }
  return out;
}

/** Whether pane `i` is the bottom of its column in the last row: its plot carries the time labels. */
export function labelled(i: number): boolean {
  if (settings.timeAxis === 'every' || (settings.timeAxis === 'auto' && window.innerWidth < 700)) return hasChart(i);
  const rs = rows();
  const last = rs[rs.length - 1] ?? [];
  return columns(last).some((c) => c[c.length - 1] === i);
}

/** The palette slot (0–7) each plotted channel has, by `file:channel`: the least-used one, and not one already in its pane. */
export const colors: Record<string, number> = $state({});

/** The palette slot of channel `ch` (its number's, until it is plotted and assigned one). */
export const colorIdx = (ch: number): number => colors[`${ui.view.file}:${ch}`] ?? ch % 8;

/** Gives every plotted channel without one a palette slot. */
export function assignColors(): void {
  const file = ui.view.file;
  if (file == null) return;
  const used = new Array<number>(8).fill(0);
  for (const p of stack.panes) for (const ch of p.channels) if (colors[`${file}:${ch}`] != null) used[colors[`${file}:${ch}`]]++;
  for (const p of stack.panes) {
    for (const ch of p.channels) {
      const k = `${file}:${ch}`;
      if (colors[k] != null) continue;
      const here = new Set(p.channels.map((c) => colors[`${file}:${c}`]));
      let best = 0;
      for (let c = 1; c < 8; c++) {
        const [a, b] = [used[best] + (here.has(best) ? 100 : 0), used[c] + (here.has(c) ? 100 : 0)];
        if (b < a) best = c;
      }
      colors[k] = best;
      used[best]++;
    }
  }
}

/** Keeps the flags consistent: the first pane starts a row, nothing is both beside and under, rows stay narrow. */
export function normalize(): void {
  let cols = 0;
  stack.panes.forEach((p, i) => {
    if (i === 0) p.beside = p.under = false;
    if (p.under && p.beside) p.beside = false;
    if (p.beside && cols >= MAX_PER_ROW) p.beside = false;
    cols = p.under ? cols : p.beside ? cols + 1 : 1;
    if (p.mode === 'chart') delete p.mode;
    if (!p.under) delete p.under;
    if (!p.beside) delete p.beside;
  });
  stack.sel = Math.max(0, Math.min(stack.sel, stack.panes.length - 1));
  assignColors();
}

/** Whether a locked view stops tables sorting and filtering. */
export const sortLocked = () => settings.locked && !settings.lockSortFilter;

function edit(f: () => void, viewOnly = false): void {
  if (!viewOnly && settings.locked && settings.lockLayout) {
    note('The layout is locked.', () => setSetting('locked', false), 'Unlock');
    return;
  }
  f();
  normalize();
  changed();
}

const rowOf = (i: number) => rows().find((r) => r.includes(i)) ?? [i];
const copy = (panes: Pane[]): Pane[] => panes.map((p) => ({ ...p, channels: [...p.channels] }));

export function addPaneBelow(i: number): void {
  const r = rowOf(i);
  edit(() => {
    stack.panes.splice(r[r.length - 1] + 1, 0, { channels: [] });
    stack.sel = r[r.length - 1] + 1;
  });
}

/** A new, empty pane below pane `i`, in its column. */
export function splitDown(i: number): void {
  edit(() => {
    stack.panes.splice(i + 1, 0, { channels: [], under: true });
    stack.sel = i + 1;
  });
}

/** A new, empty pane right of pane `i`'s column, in its row. */
export function splitRight(i: number): void {
  const col = columns(rowOf(i)).find((c) => c.includes(i)) ?? [i];
  const at = col[col.length - 1] + 1;
  edit(() => {
    stack.panes.splice(at, 0, { channels: [], beside: true });
    stack.sel = at;
  });
}

export const canSplitRight = (i: number) => columns(rowOf(i)).length < MAX_PER_ROW;

/** The same channels again, below pane `i` (or, with `beside`, right of it). */
export function duplicate(i: number, beside = false): void {
  const p = stack.panes[i];
  edit(() => {
    const again = { ...p, channels: [...p.channels], beside: undefined, under: undefined };
    stack.panes.splice(i + 1, 0, beside ? { ...again, beside: true } : { ...again, under: p.under });
    stack.sel = i + 1;
  });
}

/** Closes the panes `keep` leaves out, with Undo in the note under the top bar. */
function closeExcept(keep: (i: number) => boolean, what: string): void {
  const before = copy(stack.panes);
  const sel = stack.sel;
  const kept = stack.panes.filter((_, i) => keep(i));
  const n = stack.panes.length - kept.length;
  if (!n) return;
  const newSel = Math.max(0, kept.indexOf(stack.panes[sel]));
  edit(() => {
    stack.panes = kept;
    stack.sel = newSel;
  });
  note(`Closed ${n} pane${n === 1 ? '' : 's'} (${what}).`, () =>
    edit(() => {
      stack.panes = before;
      stack.sel = sel;
    }),
  );
}

/** The panes (numbered from 1) that plot channel `ch`. */
export const panesOf = (ch: number): number[] => stack.panes.flatMap((p, i) => (p.channels.includes(ch) ? [i + 1] : []));

/** Adds channels to pane `i` (those it has already stay), and selects it. */
export function addChannels(chs: number[], i = stack.sel): void {
  edit(() => {
    if (!stack.panes.length) stack.panes.push({ channels: [] });
    const p = stack.panes[Math.min(i, stack.panes.length - 1)];
    for (const ch of chs) if (!p.channels.includes(ch)) p.channels.push(ch);
    stack.sel = Math.min(i, stack.panes.length - 1);
  });
}

/** Moves (or, with `copy`, copies) channel `ch` from pane `from` to pane `to`, before channel `before` there (the end when null). */
export function moveChannel(ch: number, from: number, to: number, before: number | null = null, copy = false): void {
  edit(() => {
    const src = stack.panes[from];
    const dst = stack.panes[to];
    if (!src || !dst) return;
    if (!copy) src.channels = src.channels.filter((c) => c !== ch);
    dst.channels = dst.channels.filter((c) => c !== ch);
    const at = before == null ? -1 : dst.channels.indexOf(before);
    if (at >= 0) dst.channels.splice(at, 0, ch);
    else dst.channels.push(ch);
    stack.sel = to;
  });
}

/** Takes one channel out of pane `i`. */
export function removeChannel(ch: number, i: number): void {
  edit(() => {
    if (stack.panes[i]) stack.panes[i].channels = stack.panes[i].channels.filter((c) => c !== ch);
  });
}

/** Takes channels out of every pane. */
export function removeChannels(chs: number[]): void {
  edit(() => stack.panes.forEach((p) => (p.channels = p.channels.filter((c) => !chs.includes(c)))));
}

/** A new pane (a chart, or a table) at the end of the stack with these channels, selected. */
export function newPaneWith(chs: number[], mode?: PaneMode): void {
  edit(() => {
    stack.panes.push({ channels: [...chs], ...(mode && mode !== 'chart' ? { mode } : {}) });
    stack.sel = stack.panes.length - 1;
  });
}

/** Takes every channel out of every pane, keeping the panes, with Undo. */
export function clearChannels(): void {
  const before = copy(stack.panes);
  if (!before.some((p) => p.channels.length)) return;
  edit(() => stack.panes.forEach((p) => (p.channels = [])));
  note('Removed every channel from the panes.', () => edit(() => (stack.panes = before)));
}

/** Closes every pane, with Undo. */
export function clearPanes(): void {
  const before = copy(stack.panes);
  const sel = stack.sel;
  if (!before.length) return;
  edit(() => {
    stack.panes = [];
    stack.sel = 0;
  });
  note(`Removed ${before.length === 1 ? 'the pane' : `all ${before.length} panes`}.`, () =>
    edit(() => {
      stack.panes = before;
      stack.sel = sel;
    }),
  );
}

export const closePane = (i: number) => closeExcept((k) => k !== i, 'close pane');
export const closeOthers = (i: number) => closeExcept((k) => k === i, 'close others');
export function closeBelow(i: number): void {
  const r = rowOf(i);
  closeExcept((k) => k <= r[r.length - 1], 'close panes below');
}
export const hasBelow = (i: number) => {
  const r = rowOf(i);
  return r[r.length - 1] < stack.panes.length - 1;
};

export type Zone = 'left' | 'right' | 'top' | 'bottom' | 'middle';

/**
 * Drops pane `from` on pane `to`: onto its left or right edge (beside it), top or bottom (a row of its own above or
 * below), or the middle (its channels join `to`'s). `copy` leaves the original in place.
 */
export function dropPane(from: number, to: number, zone: Zone, copy = false): void {
  if (!stack.panes[from] || !stack.panes[to] || (from === to && (zone === 'middle' || copy))) return;
  edit(() => {
    const src = stack.panes[from];
    const target = stack.panes[to];
    if (zone === 'middle') {
      for (const ch of src.channels) if (!target.channels.includes(ch)) target.channels.push(ch);
      if (!copy) stack.panes.splice(from, 1);
      stack.sel = stack.panes.indexOf(target);
      return;
    }
    const moved: Pane = { ...src, channels: [...src.channels], beside: undefined, under: undefined };
    const arr = stack.panes.slice();
    if (!copy) {
      // panes under a column's top pane keep their column: the next one takes over the top's place
      const next = arr[from + 1];
      if (next?.under && !src.under) Object.assign(next, { under: undefined, beside: src.beside });
      arr.splice(from, 1);
    }
    const at = arr.indexOf(target);
    const rowStart = () => {
      let k = at;
      while (k > 0 && (arr[k].beside || arr[k].under)) k--;
      return k;
    };
    let placed = moved;
    if (zone === 'top') arr.splice(rowStart(), 0, moved);
    else if (zone === 'bottom') {
      let k = at + 1;
      while (k < arr.length && (arr[k].beside || arr[k].under)) k++;
      arr.splice(k, 0, moved);
    } else if (zone === 'right') {
      let k = at + 1;
      while (k < arr.length && arr[k].under) k++;
      placed = { ...moved, beside: true };
      arr.splice(k, 0, placed);
    } else {
      let k = at;
      while (k > 0 && arr[k].under) k--;
      const top = arr[k];
      placed = { ...moved, beside: top.beside };
      arr.splice(k, 0, placed);
      top.beside = true;
    }
    stack.panes = arr;
    stack.sel = arr.indexOf(placed);
  });
}

/** Moves pane `from` to before pane `to` (or the end), starting a row of its own there. */
export function movePane(from: number, to: number): void {
  if (from === to || from + 1 === to) return;
  edit(() => {
    const [p] = stack.panes.splice(from, 1);
    const at = to > from ? to - 1 : to;
    stack.panes.splice(at, 0, { ...p, beside: undefined, under: undefined });
    stack.sel = at;
  });
}

/** Every pane with channels as a chart, a table or both (panes of values keep their view). */
export function setAllModes(mode: 'chart' | 'table' | 'both'): void {
  edit(() => {
    for (const p of stack.panes) if (!['values', 'spectrum', 'checks', 'gauges', 'xy'].includes(p.mode ?? '')) p.mode = mode;
  });
}

/** The view every pane shares, when they do (else null). */
export function allMode(): null | 'chart' | 'table' | 'both' {
  const ms = new Set(stack.panes.filter((p) => p.channels.length && p.mode !== 'values' && p.mode !== 'spectrum' && p.mode !== 'checks').map((p) => p.mode ?? 'chart'));
  return ms.size === 1 ? ([...ms][0] as 'chart' | 'table' | 'both') : null;
}

export function setMode(i: number, mode: PaneMode): void {
  edit(() => {
    stack.panes[i].mode = mode;
    stack.sel = i;
  });
}

/** Sets the plot height of pane `i` and the panes beside it (undefined: back to the default). */
export function setPlotH(i: number, h: number | undefined): void {
  edit(() => {
    for (const k of rowOf(i)) {
      if (h == null) delete stack.panes[k].h;
      else stack.panes[k].h = Math.round(Math.max(60, Math.min(800, h)));
    }
  });
}

/** Changes what pane `i` shows for channel `name`'s widget (undefined values go back to automatic). */
export function setWidget(i: number, name: string, patch: Partial<WidgetCfg>): void {
  edit(() => {
    const p = stack.panes[i];
    if (!p) return;
    const cfg: Record<string, unknown> = { ...(p.widgets?.[name] ?? {}) };
    for (const [k, v] of Object.entries(patch)) if (v === undefined) delete cfg[k];
    else cfg[k] = v;
    const all = { ...(p.widgets ?? {}) };
    if (Object.keys(cfg).length) all[name] = cfg as WidgetCfg;
    else delete all[name];
    if (Object.keys(all).length) p.widgets = all;
    else delete p.widgets;
  });
}

/** Sets an XY pane's X channel, equal scale or local zoom (undefined: automatic). */
export function setXy(i: number, patch: Partial<Pick<Pane, 'x' | 'xyEqual' | 'xyView'>>): void {
  edit(() => {
    const p = stack.panes[i];
    if (!p) return;
    for (const [k, v] of Object.entries(patch) as [keyof typeof patch, never][]) {
      if (v === undefined) delete p[k];
      else p[k] = v;
    }
  }, true);
}

/** Sets a pane's table widths or height (undefined: back to the default). */
export function setTable(i: number, patch: Partial<Pick<Pane, 'widths' | 'tableH' | 'sort' | 'filters' | 'tfilt' | 'hideCols'>>): void {
  // sort and filters follow their own lock; widths, height and columns are layout
  const viewOnly = Object.keys(patch).every((k) => k === 'sort' || k === 'filters' || k === 'tfilt');
  if (viewOnly && sortLocked()) {
    note('Table sort and filters are locked.', () => setSetting('lockSortFilter', true), 'Allow');
    return;
  }
  edit(() => {
    const p = stack.panes[i];
    if (!p) return;
    for (const [k, v] of Object.entries(patch) as [keyof typeof patch, never][]) {
      if (v === undefined || ((k === 'widths' || k === 'filters' || k === 'tfilt') && !Object.keys(v).length) || ((k === 'sort' || k === 'hideCols') && !(v as unknown[]).length)) delete p[k];
      else p[k] = v;
    }
  }, viewOnly);
}

const unitOf = (ch: number) => ui.channels[ch]?.unit ?? '';
/** Whether pane `i` draws a chart. */
export const hasChart = (i: number) => !['values', 'table', 'spectrum', 'checks', 'gauges', 'xy'].includes(stack.panes[i]?.mode ?? 'chart');
/** Pane `i`'s units, an axis each: the first on the left, the second on the right (any more share the left). */
export const units = (i: number): string[] => [...new Set((stack.panes[i]?.channels ?? []).map(unitOf))].slice(0, 2);
/** The axis channel `ch` is drawn against in pane `i`. */
export const axisKey = (i: number, ch: number): AxisKey => (units(i).indexOf(unitOf(ch)) === 1 ? 'u1' : 'u0');
export const axisOf = (i: number, key: AxisKey): AxisSettings => stack.panes[i]?.axis?.[key] ?? {};
/** The margin right of every plot: room for a right axis's labels when any chart has one (plots stay aligned). */
export const rightPad = () => (stack.panes.some((_, i) => hasChart(i) && units(i).length > 1) ? PAD.left : PAD.right);

/** Changes one of pane `i`'s axes (undefined fields go back to the default). */
export function setAxis(i: number, key: AxisKey, patch: AxisSettings): void {
  edit(() => {
    const p = stack.panes[i];
    if (!p) return;
    const ax: AxisSettings = { ...p.axis?.[key], ...patch };
    for (const k of Object.keys(ax) as (keyof AxisSettings)[]) if (ax[k] === undefined || ax[k] === '' || ax[k] === false) delete ax[k];
    const axis = { ...p.axis, [key]: ax };
    if (!Object.keys(ax).length) delete axis[key];
    if (Object.keys(axis).length) p.axis = axis;
    else delete p.axis;
  });
}

export function resetAxes(i: number): void {
  edit(() => {
    if (stack.panes[i]) delete stack.panes[i].axis;
  });
}

/** Adds or removes channel `ch` in the selected pane (a new one when there is none). */
export function toggleChannel(ch: number): void {
  edit(() => {
    if (!stack.panes.length) stack.panes.push({ channels: [] });
    const p = stack.panes[stack.sel];
    const k = p.channels.indexOf(ch);
    if (k >= 0) p.channels.splice(k, 1);
    else p.channels.push(ch);
  });
}

let noteTimer = 0;
/** A message under the top bar, with Undo (or another `action`) when given; it goes after 12 s. */
export function note(text: string, undo?: () => void, action = 'Undo'): void {
  ui.note = text;
  ui.undo = undo ?? null;
  ui.noteAction = action;
  clearTimeout(noteTimer);
  noteTimer = window.setTimeout(() => {
    if (ui.note === text) {
      ui.note = '';
      ui.undo = null;
    }
  }, 12_000);
}
