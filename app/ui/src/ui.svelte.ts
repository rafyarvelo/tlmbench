// What the shell shows about the app's state: app.ts writes it, the components render it.

/** A listed folder or recording (the app's `list_folder`). */
export interface FolderEntry {
  path: string;
  name: string;
  dir: boolean;
  bytes: number;
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
  /** The view's time range, its span, and the pointer's time over a plot. */
  range: '',
  span: '',
  pointer: '',
  canBack: false,
  /** Both cursors are placed; any is. */
  cursors: false,
  anyCursor: false,
  /** Measure was closed by hand while cursors are placed: it stays closed until they're cleared. */
  measureDismissed: false,
  /** The last query (points, kind, time), for the status bar. */
  perf: '',
  /** A message for the person (an error, or what an edit did), shown under the top bar until closed. */
  note: '',
  /** Undoes what the note says was done. */
  undo: null as null | (() => void),
});

/** Measure shows when opened by hand, or by itself when both cursors are placed. */
export const measureShown = (open: boolean, auto: boolean) => open || (auto && ui.cursors && !ui.measureDismissed);

/** What the shell's controls do: app.ts fills these in. */
export const actions = {
  open: () => {},
  openFolder: () => {},
  openPath: (_path: string) => {},
  closeFile: () => {},
  fit: () => {},
  back: () => {},
  clearCursors: () => {},
};
