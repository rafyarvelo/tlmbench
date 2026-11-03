// The pane stack: panes in rows, a row's panes side by side in columns, a column's panes stacked (the prototype's
// model). Each pane lists its channels; `beside` puts it right of the pane before, `under` below it in the same
// column. app.ts draws into each pane's plot; PaneStack.svelte renders the panes, and these edits change them.

import { PAD } from './axes';
import { ui } from './ui.svelte';

/** How a pane shows its channels: the chart (the default, left out of saved layouts), a table of the samples in
 * view, the chart with its table below, or value tiles. */
export type PaneMode = 'chart' | 'table' | 'both' | 'values';

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
  /** The table's height under a chart ('both'), where dragged. */
  tableH?: number;
  beside?: boolean;
  under?: boolean;
}

/** Columns a row holds at most. */
export const MAX_PER_ROW = 4;

export const stack = $state({
  panes: [] as Pane[],
  sel: 0,
  /** Each channel's value at the pointer, shown in its chips. */
  values: {} as Record<number, string>,
  /** Each channel's value when the pointer is off the charts: at cursor A, else at the end of the view. */
  resting: {} as Record<number, string>,
  /** Panes drawn from summaries for now (labelled "coarse" until the exact view comes). */
  coarse: [] as boolean[],
});

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
  const rs = rows();
  const last = rs[rs.length - 1] ?? [];
  return columns(last).some((c) => c[c.length - 1] === i);
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
}

function edit(f: () => void): void {
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

export function setMode(i: number, mode: PaneMode): void {
  edit(() => {
    stack.panes[i].mode = mode;
    stack.sel = i;
  });
}

/** Sets a pane's table widths or height (undefined: back to the default). */
export function setTable(i: number, patch: Partial<Pick<Pane, 'widths' | 'tableH'>>): void {
  edit(() => {
    const p = stack.panes[i];
    if (!p) return;
    for (const [k, v] of Object.entries(patch) as [keyof typeof patch, never][]) {
      if (v === undefined || (k === 'widths' && !Object.keys(v).length)) delete p[k];
      else p[k] = v;
    }
  });
}

const unitOf = (ch: number) => ui.channels[ch]?.unit ?? '';
/** Whether pane `i` draws a chart. */
export const hasChart = (i: number) => !['values', 'table'].includes(stack.panes[i]?.mode ?? 'chart');
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
/** A message under the top bar, with Undo when given; it goes after 12 s. */
export function note(text: string, undo?: () => void): void {
  ui.note = text;
  ui.undo = undo ?? null;
  clearTimeout(noteTimer);
  noteTimer = window.setTimeout(() => {
    if (ui.note === text) {
      ui.note = '';
      ui.undo = null;
    }
  }, 12_000);
}
