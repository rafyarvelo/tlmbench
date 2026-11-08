// What the shell shows about the app's state: app.ts writes it, the components render it.

import type { FileTime } from './axes';
import type { Workspace } from './workspaces';

/** A listed folder or recording (the app's `list_folder`). */
export interface FolderEntry {
  path: string;
  name: string;
  dir: boolean;
  bytes: number;
  /** A file the app can open (undefined: yes). Others are listed only when Explorer is asked to show them. */
  openable?: boolean;
  children: FolderEntry[];
}

export const ui = $state({
  /** The open file's name and path ('' when none). */
  file: '',
  /** The folder Explorer lists (null: none open). */
  folder: null as FolderEntry | null,
  /** Its channels' names and units. */
  channels: [] as { name: string; unit: string | null }[],
  path: '',
  /** How the open file's times read: sample numbers, dates and times, or offsets from its start. */
  time: 'relative' as FileTime,
  /** The view's time range, its span, and the pointer's time over a plot. */
  range: '',
  span: '',
  pointer: '',
  /** Measure's Stats panel title: what the stats cover. */
  statsTitle: 'Stats · visible range',
  canBack: false,
  /** The start page is open over the panes (it is also what shows with no file). */
  start: false,
  /** What a values pane's tiles are read at: the pointer, cursor A or the end of the view. */
  valuesAt: '',
  /** Focus mode: only the panes and the range show. */
  focus: false,
  /** The open file's id, the time view and the cursors (exact ns as decimal strings), for table panes. */
  view: { file: null as number | null, t0: '0', t1: '1', A: null as string | null, B: null as string | null },
  /** Both cursors are placed; any is. */
  cursors: false,
  anyCursor: false,
  /** Measure was closed by hand while cursors are placed: it stays closed until they're cleared. */
  measureDismissed: false,
  /** The last query (points, kind, time), for the status bar. */
  perf: '',
  /** A message for the person (an error, or what an edit did), shown under the top bar until closed. */
  note: '',
  /** Undoes what the note says was done (or, with `noteAction`, does what that names). */
  undo: null as null | (() => void),
  noteAction: 'Undo',
  /** The views shared in Explorer's folder (`.tlmbench/views/`): each file's layout, or why it can't be read. */
  shared: [] as { file: string; ws: Workspace | null; error: string | null }[],
});

/** Measure shows when opened by hand, or by itself when both cursors are placed. */
export const measureShown = (open: boolean, auto: boolean) => open || (auto && ui.cursors && !ui.measureDismissed);

/** What the shell's controls do: app.ts fills these in. */
export const actions = {
  open: () => {},
  openFolder: () => {},
  openPath: (_path: string) => {},
  /** Opens a file and closes every other tab. */
  openOnly: (_path: string) => {},
  closeFile: () => {},
  showTab: (_key: number) => {},
  closeTab: (_key: number) => {},
  closeOtherTabs: (_key: number) => {},
  /** Closes a file: every tab showing it. */
  closePath: (_path: string) => {},
  /** Opens a folder's session ('' : the files opened without a folder): its folder in Explorer and its tabs. */
  openSession: (_folder: string) => {},
  /** Opens several files, each in a tab of its own (the last in front). */
  openPaths: (_paths: string[]) => {},
  /** Closes several files, with Undo. */
  closePaths: (_paths: string[]) => {},
  saveSession: () => {},
  /** The native file picker for these extensions; null when cancelled. */
  pickFile: async (_ext: string[]): Promise<string | null> => null,
  fit: () => {},
  toggleTheme: () => {},
  toggleFocus: (_on?: boolean) => {},
  back: () => {},
  clearCursors: () => {},
  /** Fills Measure's panels again (one was just shown or moved). */
  refreshMeasure: () => {},
  /** Places cursor A or B at a time (a table row). */
  placeCursor: (_which: 'A' | 'B', _t: bigint) => {},
  /** Centres the time view on a time, keeping its span. */
  centreOn: (_t: bigint) => {},
  /** Workspaces: Save as… (a layout of tab `key`, which then shows it; an error to show, or null), Save layout,
   * Revert changes, Delete layout, open one in a tab (on the file in front), and a new empty one. */
  saveAs: (_key: number, _name: string): string | null => null,
  saveWs: (_key: number) => {},
  revertWs: (_key: number) => {},
  deleteWs: (_name: string) => {},
  openWs: (_name: string) => {},
  newWs: (_name: string): string | null => null,
  /** Shared views: list the folder's again, open one, share tab `key`'s layout under a name, export it to a file. */
  loadShared: () => {},
  openShared: (_file: string) => {},
  share: (_key: number, _name: string): string | null => null,
  exportWs: (_key: number) => {},
  /** What a pane's axis shows now, in its units (null before it's drawn). */
  axisRange: (_i: number, _key: 'u0' | 'u1'): [number, number] | null => null,
};
