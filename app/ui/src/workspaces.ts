// Workspaces: saved layouts (design: Files, workspaces and sharing). A layout names its channels instead of numbering
// them, so it applies to any recording that has them; yours are kept in the app's settings, never beside the data.
// The format is the one shared views use (`.tlmbench/views/*.tlmview.json`).

import type { AxisKey, AxisSettings, Pane, PaneMode } from './stack.svelte';

export interface WsPane {
  channels: string[];
  beside?: boolean;
  under?: boolean;
  /** The pane's view; left out for a chart. */
  view?: PaneMode;
  axis?: Partial<Record<AxisKey, AxisSettings>>;
  /** Table column widths: 'n' and 't' for sample and time, 'c:<channel name>' for a channel. */
  widths?: Record<string, number>;
  tableH?: number;
  /** Table sort and filters, with columns named as `widths` does. */
  sort?: { col: string; desc: boolean }[];
  filters?: Record<string, string>;
}

export interface Workspace {
  schema: 1;
  name: string;
  /** Who shared it (shared views only). */
  by?: string;
  /** A shared view's files, by path relative to its folder (slot 1 is the primary file). */
  files?: { path: string; slot: number }[];
  /** A shared view's time range: seconds from the primary file's first sample. */
  view?: [number, number];
  panes: WsPane[];
}

/** A shared view's file name for a layout name: `flight-review.tlmview.json`. */
export const viewFile = (name: string) => `${name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'view'}.tlmview.json`;

/** A layout of `panes` over a recording whose channels are `names`. */
export function toWorkspace(name: string, panes: Pane[], names: string[]): Workspace {
  return {
    schema: 1,
    name,
    panes: panes.map((p) => {
      const w: WsPane = { channels: p.channels.map((ch) => names[ch]).filter((n) => n != null) };
      if (p.beside) w.beside = true;
      if (p.under) w.under = true;
      if (p.mode && p.mode !== 'chart') w.view = p.mode;
      if (p.axis) w.axis = structuredClone(p.axis);
      if (p.widths) w.widths = Object.fromEntries(Object.entries(p.widths).map(([k, v]) => [k.startsWith('c') ? `c:${names[Number(k.slice(1))]}` : k, v]));
      if (p.tableH != null) w.tableH = p.tableH;
      const colName = (c: string) => (c.startsWith('c') ? `c:${names[Number(c.slice(1))]}` : c);
      if (p.sort?.length) w.sort = p.sort.map((k) => ({ col: colName(k.col), desc: k.desc }));
      if (p.filters) w.filters = Object.fromEntries(Object.entries(p.filters).map(([k, v]) => [colName(k), v]));
      return w;
    }),
  };
}

/** `ws` over a recording whose channels are `names`: its panes, and the channels it names that the recording lacks. */
export function fromWorkspace(ws: Workspace, names: string[]): { panes: Pane[]; missing: string[] } {
  const index = new Map(names.map((n, i) => [n, i]));
  const missing = new Set<string>();
  const panes = ws.panes.map((w) => {
    const channels: number[] = [];
    for (const n of w.channels ?? []) {
      const i = index.get(n);
      if (i == null) missing.add(n);
      else if (!channels.includes(i)) channels.push(i);
    }
    const p: Pane = { channels };
    if (w.beside) p.beside = true;
    if (w.under) p.under = true;
    if (w.view && w.view !== 'chart') p.mode = w.view;
    if (w.axis) p.axis = structuredClone(w.axis);
    if (w.widths) {
      const widths: Record<string, number> = {};
      for (const [k, v] of Object.entries(w.widths)) {
        const id = k.startsWith('c:') ? index.get(k.slice(2)) : null;
        if (!k.startsWith('c:')) widths[k] = v;
        else if (id != null) widths[`c${id}`] = v;
      }
      if (Object.keys(widths).length) p.widths = widths;
    }
    if (w.tableH != null) p.tableH = w.tableH;
    const colId = (c: string) => (c.startsWith('c:') ? (index.has(c.slice(2)) ? `c${index.get(c.slice(2))}` : null) : c);
    const sort = (w.sort ?? []).flatMap((k) => (colId(k.col) ? [{ col: colId(k.col)!, desc: !!k.desc }] : []));
    if (sort.length) p.sort = sort;
    const filters = Object.fromEntries(Object.entries(w.filters ?? {}).flatMap(([k, v]) => (colId(k) ? [[colId(k)!, v]] : [])));
    if (Object.keys(filters).length) p.filters = filters;
    return p;
  });
  return { panes, missing: [...missing] };
}

/** Whether `ws` is a layout this app can open (a shared file may hold anything). */
export function isWorkspace(ws: unknown): ws is Workspace {
  const w = ws as Workspace;
  return !!w && typeof w.name === 'string' && Array.isArray(w.panes) && w.panes.every((p) => Array.isArray(p?.channels) && p.channels.every((c) => typeof c === 'string'));
}
