// Play: a playhead moves through the file at a chosen speed and the view follows it (design: Time modes). Times
// here are seconds from the file's first sample; app.ts moves the playhead each frame and draws it.

/** Speeds offered, in seconds of recording a second. */
export const SPEEDS = [0.25, 0.5, 1, 2, 5, 10, 30, 60, 300, 600, 3600, 86400];
/** Window presets, in seconds. */
export const WINDOWS = [10, 60, 600, 3600];

export const speedLabel = (s: number) =>
  s >= 86400 ? `${s / 86400} day/s` : s >= 3600 ? `${s / 3600} h/s` : s > 60 && s % 60 === 0 ? `${s / 60} min/s` : `${s}×`;

export const play = $state({
  /** The transport is shown (the tab is in Play). */
  open: false,
  running: false,
  /** The playhead, and the file's length. */
  t: 0,
  end: 1,
  speed: 1,
  /** The window the view follows with, in seconds. */
  win: 60,
  /** The view follows the playhead (any view you choose stops it; Follow brings it back). */
  follow: true,
  /** Repeat the stretch between cursors A and B. */
  loop: false,
  /** Both cursors are placed (the loop needs them). */
  canLoop: false,
});

/** What Stop at is set to: `off`, `any`, or `cat:id` for chosen categories (several are joined with commas). */
export function stopCats(v: string): 'off' | 'any' | string[] {
  if (v === 'off' || v === 'any' || !v) return v === 'any' ? 'any' : 'off';
  const ids = v.split(',').filter((x) => x.startsWith('cat:')).map((x) => x.slice(4));
  return ids.length ? ids : 'off';
}

/** The setting for these chosen categories (none: off). */
export const stopValue = (ids: string[]): string => (ids.length ? ids.map((i) => `cat:${i}`).join(',') : 'off');

/** What the transport's controls do: app.ts fills these in. */
export const playActions = {
  /** Opens the transport, paused. */
  open: () => {},
  toggle: () => {},
  seek: (_t: number) => {},
  setWindow: (_s: number) => {},
  follow: () => {},
  close: () => {},
};
