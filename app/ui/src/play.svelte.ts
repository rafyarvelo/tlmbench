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
