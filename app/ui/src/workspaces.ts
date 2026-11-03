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
}

export interface Workspace {
  schema: 1;
  name: string;
  /** Who shared it (shared views only). */
  by?: string;
  panes: WsPane[];
}

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
    return p;
  });
  return { panes, missing: [...missing] };
}

/** Whether `ws` is a layout this app can open (a shared file may hold anything). */
export function isWorkspace(ws: unknown): ws is Workspace {
  const w = ws as Workspace;
  return !!w && typeof w.name === 'string' && Array.isArray(w.panes) && w.panes.every((p) => Array.isArray(p?.channels) && p.channels.every((c) => typeof c === 'string'));
}
