// What the shell shows about the app's state: app.ts writes it, the components render it.

import type { FileTime } from './axes';
import type { Check, CheckSet, Outcome } from './checks.svelte';
import type { TlmEvent } from './events.svelte';
import type { Workspace } from './workspaces';

/** A listed folder or recording (the app's `list_folder`). */
export interface FolderEntry {
  path: string;
  name: string;
  dir: boolean;
  bytes: number;
  /** A file the app can open (undefined: yes). Others are listed only when Explorer is asked to show them. */
  openable?: boolean;
  /** A binary file no descriptor names: Explorer says so, and opening it asks for one. */
  needsDescriptor?: boolean;
  children: FolderEntry[];
}

/** How a file of a tab lines up with the primary (file 1). */
export type AlignMethod = 'starts' | 'clock' | 'manual' | 'sync' | 'events' | 'xcorr' | 'timesrc';
export interface AlignView {
  method: AlignMethod;
  /** Manual: seconds after the primary's start. */
  seconds: number;
  /** The channel matched on (cross-correlation, time channel). */
  channel?: string;
  /** One line from the method (`Matched on accel_x: +1.2 s, r = 0.98`) or why it failed. */
  note?: string;
  /** 0..1, for methods that fit something. */
  quality?: number;
  /** `[t_file_s, residual_ms]` per matched point. */
  residuals: [number, number][];
  /** Sync points: pairs of own time and primary time, in seconds from the timeline's start. */
  pairs: [number, number][];
  locked: boolean;
  /** The offset now, in seconds (the first knot's), and the drift in ppm. */
  offset: number;
  drift: number;
  /** A result waiting to be applied (the overlay preview). */
  preview?: { knots: [string, string][] } | null;
}
/** A file of the tab in front when it has several: its slot (1 is the primary), short name and alignment. */
export interface SlotRow {
  slot: number;
  file: number;
  path: string;
  name: string;
  /** Its first channel in the tab's numbering, and how many it has. */
  first: number;
  count: number;
  align: AlignView;
}

export const ui = $state({
  /** The files of the tab in front when it holds several (empty: one file). */
  slots: [] as SlotRow[],
  /** The open file's name and path ('' when none). */
  file: '',
  /** The folder Explorer lists (null: none open). */
  folder: null as FolderEntry | null,
  /** Its channels' names and units. */
  channels: [] as { name: string; unit: string | null; category?: string | null; /** Samples called missing in this file. */ missing?: number; /** A derived channel: its recipe's expression. */ derived?: { expr: string; automatic: boolean } }[],
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
  /** The time widgets and tiles read at (ns): the pointer, the playhead, cursor A or the end of the view; null with no file. */
  readAt: null as string | null,
  /** The pointer's time over a chart (ns), for panes that mark it (XY). */
  hover: null as string | null,
  /** Focus mode: only the panes and the range show. */
  focus: false,
  /** The open file's id, the time view and the cursors (exact ns as decimal strings), for table panes. */
  view: { file: null as number | null, t0: '0', t1: '1', first: '0', end: '1', A: null as string | null, B: null as string | null },
  /** What the charts mark: where the filter of table pane `pane` matches, and the rows Apply to charts leaves out,
   * as runs of time (ns). */
  marks: null as null | { pane: number; hits: [bigint, bigint][]; excluded: [bigint, bigint][]; overview: [bigint, bigint][] },
  /** The time span of the rows each table pane shows, by pane: a shaded band on the charts. */
  bands: {} as Record<number, [bigint, bigint]>,
  /** A table's Apply to charts is on: how many rows it leaves out, and the pane that holds it. */
  filtered: null as null | { rows: number; pane: number },
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
  /** Closes the folder's tabs and forgets its saved session. */
  resetSession: () => {},
  /** Opens a folder's session ('' : the files opened without a folder): its folder in Explorer and its tabs. */
  openSession: (_folder: string) => {},
  /** Opens several files, each in a tab of its own (the last in front). */
  openPaths: (_paths: string[]) => {},
  /** Several files in one tab: add a file to the tab in front, close one of them, make one the primary. */
  addToTab: (_path: string) => {},
  closeSlot: (_slot: number) => {},
  makePrimary: (_slot: number) => {},
  /** A file's alignment: choose a method (with its parameters), nudge it, lock it, add a sync point from cursors A and B. */
  alignSet: (_slot: number, _method: AlignMethod, _arg?: { seconds?: number; channel?: string }) => {},
  alignApply: (_slot: number) => {},
  alignDiscard: (_slot: number) => {},
  alignNudge: (_slot: number, _seconds: number) => {},
  alignLock: (_slot: number, _on: boolean) => {},
  alignPair: (_slot: number) => {},
  alignPairs: (_slot: number, _pairs: [number, number][]) => {},
  /** Channel prefix…: what a file's channels are called in legends and menus ('' for its name). */
  setPrefix: (_path: string, _prefix: string) => {},
  /** Compare runs: two files in one tab with a pane per shared channel. */
  compare: (_a: string, _b: string) => {},
  /** Shows a file's import dialog again (its saved settings first); a recording that isn't a CSV opens as it is. */
  importAgain: (_path: string) => {},
  /** The import dialog: Open / Add to Explorer, and Cancel. */
  /** Import all: the dialog for the files whose headers have no saved settings, opening nothing. */
  importAll: (_paths: string[]) => {},
  importGo: () => {},
  importCancel: () => {},
  /** Export data… and Save the view as an image…: open the dialogs, write the files. */
  /** Derive…: a new channel from these channels (none, one, or two), editing a derived channel's recipe, removing it. */
  deriveOpen: (_chs: number[]) => {},
  deriveEdit: (_ch: number) => {},
  deriveTemplate: (_id: string) => {},
  deriveExpr: (_text: string) => {},
  deriveSubmit: () => {},
  deriveClose: () => {},
  deriveRemove: (_ch: number, _from: 'file' | 'all') => {},
  /** Events: add a marker at cursor A or a range A–B to the open file, save an edited one, delete one, frame one in the view. */
  eventAdd: (_range: boolean) => {},
  /** A pane's event markers were switched: draw and save the layout. */
  eventsRedraw: () => {},
  eventSave: (_e: TlmEvent) => {},
  eventDelete: (_id: string) => {},
  eventShow: (_id: string) => {},
  /** Rules & checks: keep a set (the name it had before, for an edit; an error to show, or null), delete one, run one
   *  row on the file in front (the editor's preview), frame a violation, open the editor on a set (null: a new one) at a
   *  row, run everything that applies again. */
  checkSave: async (_set: CheckSet, _was: string | null): Promise<string | null> => null,
  checkDelete: (_name: string) => {},
  checkPreview: async (_c: Check): Promise<Outcome | null> => null,
  checkShow: (_start: string, _end: string) => {},
  checkEditor: (_name: string | null, _focus?: number, _rule?: boolean) => {},
  checkRun: () => {},
  /** The folder run: run a set on Explorer's folder, write its results as CSV, open a grid cell's file at its first violation in the group. */
  checkFolderRun: (_set: string) => {},
  checkFolderCsv: () => {},
  checkOpenCell: (_path: string, _group: string) => {},
  /** The Recipes manager: open, close, edit / duplicate / delete a recipe, automatic on or off, Export… and Import… text. */
  recipesOpen: () => {},
  recipesClose: () => {},
  recipeEdit: (_name: string) => {},
  recipeDuplicate: (_name: string) => {},
  recipeDelete: (_name: string) => {},
  recipeAuto: (_name: string, _on: boolean) => {},
  recipesExport: () => {},
  recipesImport: (_text: string) => {},
  exportOpen: () => {},
  exportGo: () => {},
  exportClose: () => {},
  imageOpen: () => {},
  imageGo: () => {},
  imageClose: () => {},
  /** The descriptor form for a file: the descriptor that names it, else a new one. */
  descriptorForm: (_path: string) => {},
  dscPick: (_id: string) => {},
  dscPreview: () => {},
  dscFind: () => {},
  dscSave: () => {},
  dscOpen: () => {},
  dscShareGo: () => {},
  dscClose: () => {},
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
  /** Sets the time view (an XY pane's Alt-drag); refused while the view is locked. */
  setTimeView: (_t0: bigint, _t1: bigint) => {},
  /** The charts, overview and stats ask again (after what the core leaves out of them changed). */
  refreshViews: () => {},
  /** The pointer is over a table row (a time), or left the table (null): the charts show it as a hover line. */
  hoverAt: (_t: bigint | null) => {},
  /** The charts draw again (their marks changed). */
  redraw: () => {},
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
