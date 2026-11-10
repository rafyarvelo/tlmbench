// Tabs: a file tab per open file, and workspace tabs showing a saved layout on a file (each keeps its panes, view,
// cursors and history while another tab is in front).
// The strip shows `tabs.list`; app.ts keeps each tab's state and switches between them (`actions` in ui.svelte.ts).

export interface TabInfo {
  key: number;
  path: string;
  name: string;
  /** The packet type this tab shows, for a file with several (its name reads `flight.bin · gps`). */
  part?: string;
  /** A live source's tab (its file grows); `path` is the source's endpoint. */
  live?: boolean;
  /** A workspace tab: the name of the layout it shows (`name` too), on the file at `path`. */
  ws?: string;
  /** A shared view's tab: its file in the folder's `.tlmbench/views/` (read-only; Save as… makes it yours). */
  shared?: string;
  /** An imported (CSV) file's data rate as text (Explorer's chip; a click shows its import settings). */
  rate?: string;
  /** A Compare runs tab: named `a vs b`. */
  compare?: boolean;
  /** Its panes differ from the layout as saved. */
  dirty?: boolean;
}

export const tabs = $state({
  list: [] as TabInfo[],
  /** The key of the tab in front (-1: none). */
  active: -1,
});

/** Moves tab `from` (an index) to before index `to` (or the end). */
export function moveTab(from: number, to: number): void {
  if (from === to || from + 1 === to) return;
  const [t] = tabs.list.splice(from, 1);
  tabs.list.splice(to > from ? to - 1 : to, 0, t);
}
