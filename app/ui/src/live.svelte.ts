// Live: the tab in front follows a source as it records (design: Live streaming). A live tab is a file tab whose
// file grows; this is what its strip shows. app.ts polls the source and moves the view.

/** A source's stream health (tlmbench_live::Health). */
export interface Health {
  endpoint: string;
  bytes_in: number;
  records_in: number;
  records_dropped: number;
  records_written: number;
  sequence_missing: number;
  sequence_repeated: number;
  malformed_bytes: number;
  rows_skipped: number;
  rows_back_in_time: number;
  files: string[];
  ring_bytes: number;
  ring_high_water_bytes: number;
  ended: boolean;
  error: string | null;
  seconds: number;
}

/** Window presets, in seconds. */
export const LIVE_WINDOWS = [10, 30, 60, 300];

export const live = $state({
  /** The tab in front is a live one. */
  on: false,
  /** The view follows now (a view you choose holds it; Jump to live comes back). */
  following: true,
  win: 30,
  /** Seconds recorded, and how far back the view's end is (0 when following). */
  recorded: 0,
  back: 0,
  health: null as Health | null,
  session: '',
  /** The Sources panel is open. */
  sources: false,
  /** The Connect dialog is open, and why the last connect failed. */
  connect: false,
  error: '',
  connecting: false,
});

/** What the strip and the dialog do: app.ts fills these in. */
export const liveActions = {
  connect: (_endpoint: string, _descriptor: string) => {},
  setWindow: (_s: number) => {},
  jump: () => {},
  scrub: (_fraction: number) => {},
  stop: () => {},
};
