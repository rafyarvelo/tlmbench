// Settings: everything configurable, saved in the app's local storage as it changes (CLAUDE.md: production-like).
// Components and app.ts read `settings` directly; changes go through `setSetting` so they are saved and applied.

import type { Pane } from './stack.svelte';
import type { Workspace } from './workspaces';

export type Mode = 'box' | 'pan' | 'cursor';
export type Theme = 'system' | 'light' | 'dark';
export type ZoomSpeed = 'slow' | 'normal' | 'fast';
/** What the left sidebar shows (empty: hidden). */
export type LeftPanel = 'explorer' | 'settings' | '';

export interface Settings {
  mode: Mode;
  theme: Theme;
  zoomSpeed: ZoomSpeed;
  dblFit: boolean;
  overviewTop: boolean;
  leftPanel: LeftPanel;
  /** Measure opened by hand (it also opens by itself when both cursors are placed, with measureAuto). */
  measureOpen: boolean;
  measureAuto: boolean;
  showPerf: boolean;
  /** Settings shows every setting, not just the everyday ones. */
  settingsAll: boolean;
  /** The folder Explorer lists, and its folders (and open files) shown expanded. */
  folder: string;
  exExpanded: string[];
  /** The tabs open when the app last closed (their files, then layouts shown on them) and the one in front. */
  session: { files: string[]; active: string | null; ws?: { path: string; name: string; at: number }[]; activeWs?: string };
  /** Your layouts (workspaces), this computer only. */
  workspaces: Workspace[];
  /** Each file's panes (their channel numbers and where they sit), by file path. */
  layouts: Record<string, Pane[]>;
  /** Gap factors set by hand, by file path and channel number (the rest use the default). */
  gapFactors: Record<string, Record<number, number>>;
}

export const DEFAULTS: Settings = {
  mode: 'box',
  theme: 'system',
  zoomSpeed: 'normal',
  dblFit: true,
  overviewTop: false,
  leftPanel: 'explorer',
  measureOpen: false,
  measureAuto: true,
  showPerf: false,
  settingsAll: false,
  folder: '',
  exExpanded: [],
  session: { files: [], active: null },
  layouts: {},
  workspaces: [],
  gapFactors: {},
};

const KEY = 'tlmbench.settings';

function load(): Settings {
  try {
    return { ...structuredClone(DEFAULTS), ...JSON.parse(localStorage.getItem(KEY) ?? '{}') };
  } catch {
    return structuredClone(DEFAULTS);
  }
}

export const settings: Settings = $state(load());

const listeners = new Set<(key: keyof Settings) => void>();

/** Calls `f` with each setting's name after it changes. */
export function onSetting(f: (key: keyof Settings) => void): void {
  listeners.add(f);
}

export function saveSettings(): void {
  try {
    localStorage.setItem(KEY, JSON.stringify($state.snapshot(settings)));
  } catch {
    /* private mode: settings last this session */
  }
}

export function setSetting<K extends keyof Settings>(key: K, value: Settings[K]): void {
  settings[key] = value;
  saveSettings();
  listeners.forEach((f) => f(key));
}

/** What Reset to defaults keeps: the panes and gap factors remembered per file, and where the sidebars are. */
const KEEP: (keyof Settings)[] = ['layouts', 'workspaces', 'gapFactors', 'leftPanel', 'measureOpen', 'settingsAll', 'folder', 'exExpanded', 'session'];

export function resetSettings(): void {
  for (const key of Object.keys(DEFAULTS) as (keyof Settings)[]) {
    if (!KEEP.includes(key)) setSetting(key, structuredClone(DEFAULTS[key]) as never);
  }
}

/** The theme on the page: Match system leaves it to the colour scheme. */
export function applyTheme(): void {
  const t = settings.theme;
  if (t === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = t;
}
