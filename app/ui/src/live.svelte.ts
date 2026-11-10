// Live: a tab whose file grows, its view following now (design: Live streaming). A live tab is a file tab whose file
// grows; this is what its strip shows. app.ts polls the source and moves the view.
import type { ConnectForm, TriggerForm } from './settings.svelte';
import { tabs } from './tabs.svelte';

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
  paused: boolean;
  seconds: number;
}

/** Where a trigger stands (tlmbench_store::trigger::Status). */
export interface TriggerStatus {
  armed: boolean;
  count: number;
  /** The latest crossing, ns as text. */
  last: string | null;
  /** Fired, and the recording hasn't reached its post time yet. */
  pending: boolean;
  error: string | null;
  spec: (Omit<TriggerForm, 'channel'> & { channel: string }) | null;
}

/** One source of the session, as its card shows it. */
export interface SourceCard {
  key: number;
  name: string;
  endpoint: string;
  health: Health;
  /** The source was stopped by hand (else it ended by itself, or runs). */
  stopped: boolean;
  /** A replay, whose pause is a real pause. */
  replay: boolean;
  /** Where its session is recorded, and whether it is kept. */
  session: string;
  recording: boolean;
}

/** Window presets, in seconds. */
export const LIVE_WINDOWS = [10, 30, 60, 300];

/** Speeds a file playback runs at. */
export const LIVE_SPEEDS = [0.5, 1, 2, 5, 10, 50];

/** The state a source is in, for its badge and card. */
export function stateOf(h: Health | null, stopped: boolean): 'streaming' | 'paused' | 'stopped' | 'ended' {
  if (!h) return 'streaming';
  if (stopped) return 'stopped';
  if (h.ended) return 'ended';
  return h.paused ? 'paused' : 'streaming';
}

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
  /** What this session's source is called. */
  name: '',
  /** The recording is kept (Record while streaming was on). */
  recording: true,
  /** The source was stopped by hand. */
  stopped: false,
  /** The source in front is a replay of a file. */
  replay: false,
  trigger: null as TriggerStatus | null,
  /** The Sources panel is open. */
  sources: false,
  /** Every live tab's source (the Sources panel's cards). */
  cards: [] as SourceCard[],
  /** The Connect dialog is open, and why the last connect failed. */
  connect: false,
  /** Connecting from a live tab adds a source to its session (the tab's key), or opens a tab of its own (null). */
  addTo: null as number | null,
  /** The form the dialog opens with (a saved connection), and the one it was saved as. */
  preset: null as (Partial<ConnectForm> & { id?: string }) | null,
  error: '',
  connecting: false,
  /** The session drawn small behind the scrubber: points in 0..1 across and 0..1 up, as an SVG path. */
  trace: '',
});

/** What a Connect dialog hands over. */
export interface ConnectArgs {
  endpoint: string;
  descriptor: string;
  name: string;
  record: boolean;
}

/** What the strip and the dialog do: app.ts fills these in. */
export const liveActions = {
  connect: (_a: ConnectArgs, _save: boolean, _form: ConnectForm) => {},
  setWindow: (_s: number) => {},
  jump: () => {},
  scrub: (_fraction: number) => {},
  stop: (_key?: number) => {},
  /** Pauses or resumes a source (the one in front when no key). */
  pause: (_key?: number) => {},
  /** Closes a source's tab. */
  remove: (_key: number) => {},
  /** Opens a source's tab. */
  show: (_key: number) => {},
  /** Arms a trigger, or stops it (null); returns why it couldn't. */
  trigger: async (_spec: TriggerForm | null): Promise<string | null> => null,
  preview: async (_endpoint: string, _descriptor: string): Promise<{ detail: string; channels: [string, string | null][] } | string> => '',
  /** Opens the Connect dialog: for a session in front, "+ Add source" (its source joins it). */
  openConnect: (add: boolean, preset?: Partial<ConnectForm> & { id?: string }) => {
    live.addTo = add ? tabs.active : null;
    live.preset = preset ?? null;
    live.error = '';
    live.connect = true;
  },
};
