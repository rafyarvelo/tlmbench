// The export dialogs' state: Export data… (samples of this tab's channels, written by the core's `convert` writers) and
// Save the view as an image…. The dialogs capture what the tab shows when they open; the core writes the files.

export const FORMATS: Record<string, [string, string]> = {
  csv: ['CSV', 'csv'],
  parquet: ['Parquet', 'parquet'],
  arrow: ['Arrow IPC', 'arrow'],
  hdf5: ['HDF5', 'h5'],
  mat: ['MAT v7.3 (MATLAB)', 'mat'],
};

/** What the front tab shows when Export data… opens. */
export interface ExportContext {
  /** The file's name without its extension, and the packet type (`flight.gps`). */
  base: string;
  /** Channels in this tab's panes, and every channel of the file: [number, name]. */
  tab: [number, string][];
  all: [number, string][];
  /** The time view, the cursors (null: not both placed) and the whole recording, in nanoseconds. */
  view: [bigint, bigint];
  cursors: [bigint, bigint] | null;
  whole: [bigint, bigint];
  samples: number;
}

export const exp = $state({
  open: false,
  busy: false,
  error: '',
  result: '',
  ctx: null as ExportContext | null,
});

/** Export to HTML…: the sizes the core estimates for Full and Light, and the outcome. */
export const html = $state({
  open: false,
  busy: false,
  error: '',
  result: '',
  ctx: null as ExportContext | null,
  /** Bytes of the folder in each mode for the chosen channels, and the budget. */
  sizes: null as { full: number; light: number; budget: number } | null,
  /** Channels of the layout in front, and those it shows as a table, values or XY (raw samples in Light). */
  layout: [] as number[],
  raw: [] as number[],
  sets: [] as string[],
});

export const img = $state({ open: false, busy: false, error: '', result: '', panes: 0, sel: 0, base: '' });
