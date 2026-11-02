// The pane stack: panes in rows, a row's panes side by side in columns, a column's panes stacked (the prototype's
// model). Each pane lists its channels; `beside` puts it right of the pane before, `under` below it in the same
// column. app.ts draws into each pane's plot; PaneStack.svelte renders the panes, and these edits change them.

import { ui } from './ui.svelte';

export interface Pane {
  channels: number[];
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
    stack.panes.splice(i + 1, 0, beside ? { channels: [...p.channels], beside: true } : { channels: [...p.channels], under: p.under });
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
    stack.panes.splice(at, 0, { channels: p.channels });
    stack.sel = at;
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
