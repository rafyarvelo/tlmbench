// Tabs: one per open file (a file tab: its panes, view, cursors and history, kept while another tab is in front).
// The strip shows `tabs.list`; app.ts keeps each tab's state and switches between them (`actions` in ui.svelte.ts).

export interface TabInfo {
  key: number;
  path: string;
  name: string;
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
