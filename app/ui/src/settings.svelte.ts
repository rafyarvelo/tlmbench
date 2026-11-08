// Settings: everything configurable, saved in the app's local storage as it changes (CLAUDE.md: production-like).
// Components and app.ts read `settings` directly; changes go through `setSetting` so they are saved and applied.

import type { TimeFormat } from './axes';
import type { Pane } from './stack.svelte';
import type { Workspace } from './workspaces';

export type Mode = 'box' | 'pan' | 'cursor';
export type Theme = 'system' | 'light' | 'dark';
export type Tooltips = 'short' | 'full' | 'off';
/** The panes a file opens with. */
export type StarterPanes = 'starter' | 'starterboth' | 'table' | 'values' | 'none';
export type TimeAxis = 'auto' | 'bottom' | 'every';
export type ZoomSpeed = 'slow' | 'normal' | 'fast';
/** What the left sidebar shows, a rail group (empty: hidden). */
export type LeftPanel = 'explorer' | 'settings' | 'measure' | '';
/** A sidebar panel: Explorer, Settings, and Measure's Cursors and Stats. */
export type PanelId = 'explorer' | 'settings' | 'cur' | 'stats';
/** The sidebars' panels, top to bottom (owner, Nov 7: they drag by their title to reorder or change sides). */
export interface PanelLayout {
  left: PanelId[];
  right: PanelId[];
}
/** Each panel's rail group: the left sidebar shows one group at a time, the right one all its panels. */
export const GROUP_OF: Record<PanelId, Exclude<LeftPanel, ''>> = { explorer: 'explorer', settings: 'settings', cur: 'measure', stats: 'measure' };
export const PANEL_TITLE: Record<PanelId, string> = { explorer: 'Explorer', settings: 'Settings', cur: 'Cursors', stats: 'Stats' };

export interface Settings {
  mode: Mode;
  theme: Theme;
  zoomSpeed: ZoomSpeed;
  dblFit: boolean;
  /** The time view can't be changed by gestures; cursors still work unless `lockCursors` is off. */
  locked: boolean;
  lockCursors: boolean;
  /** A locked view also refuses edits of the panes. */
  lockLayout: boolean;
  /** The wheel zooms the charts (off: Ctrl+wheel zooms and the wheel pans). */
  wheelZoom: boolean;
  /** A channel dropped where a new pane goes makes a chart, or the same view as the pane it came from. */
  dropNewView: 'chart' | 'same';
  /** How-to notes in the panels. */
  showHints: boolean;
  overviewTop: boolean;
  leftPanel: LeftPanel;
  /** Sidebar widths in px (dragged at their inner edge), sidebar panels collapsed (by name), and Explorer and Measure traded sides. */
  leftWidth: number;
  measureWidth: number;
  panelsClosed: string[];
  /** How times read (axes, readouts, tables, cursors), their decimals ('auto' or 0–6), and which charts label the time axis. */
  timeFormat: TimeFormat;
  timeDecimals: string;
  timeAxis: TimeAxis;
  /** When a file opens: its remembered panes ('remember') or a starter; and the starter for a file opened for the first time. */
  fileOpen: 'remember' | StarterPanes;
  fileOpenNew: StarterPanes;
  /** Explorer rows: select first, or act at once. */
  clickMode: 'select' | 'act';
  /** Hides badges and counts until they matter. */
  quietUI: boolean;
  /** The channel the overview is pinned to (null: it follows the selected pane). */
  overviewCh: number | null;
  /** Tooltips (Short retires each after three showings; `tipsSeen` counts them by tip). */
  tooltips: Tooltips;
  tipsSeen: Record<string, number>;
  /** The start page shows at launch, and the recordings opened last (newest first). */
  showStart: boolean;
  recent: string[];
  /** Explorer groups channels into categories (by name prefix), and the categories folded shut. */
  exCategories: boolean;
  exCatClosed: string[];
  /** Channels moved to a category of your own, by file path and channel name. */
  catOverride: Record<string, string>;
  /** Open files whose channels are folded away in Explorer. */
  exFilesShut: string[];
  /** Explorer also lists the files the app can't open. */
  exShowAll: boolean;
  /** Explorer lists only the open files. */
  exOnlyOpen: boolean;
  panelLayout: PanelLayout;
  /** The right sidebar opened by hand (with Measure's panels on it, it also opens by itself when both cursors are
   *  placed, with measureAuto). */
  measureOpen: boolean;
  measureAuto: boolean;
  showPerf: boolean;
  /** Settings shows every setting, not just the everyday ones. */
  settingsAll: boolean;
  /** The folder Explorer lists, and its folders (and open files) shown expanded. */
  folder: string;
  exExpanded: string[];
  /** The tabs open when the app last closed (their files, then layouts shown on them) and the one in front. */
  session: { files: string[]; active: string | null; ws?: { path: string; name: string; at: number; shared?: string }[]; activeWs?: string };
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
  locked: false,
  lockCursors: true,
  lockLayout: false,
  wheelZoom: true,
  dropNewView: 'chart',
  showHints: true,
  overviewTop: false,
  leftPanel: 'explorer',
  leftWidth: 300,
  measureWidth: 260,
  panelsClosed: [],
  timeFormat: 'auto',
  timeDecimals: 'auto',
  timeAxis: 'auto',
  fileOpen: 'remember',
  fileOpenNew: 'starter',
  clickMode: 'select',
  quietUI: false,
  overviewCh: null,
  tooltips: 'short',
  tipsSeen: {},
  showStart: true,
  recent: [],
  exCategories: true,
  exCatClosed: [],
  catOverride: {},
  exFilesShut: [],
  exShowAll: false,
  exOnlyOpen: false,
  panelLayout: { left: ['explorer', 'settings'], right: ['cur', 'stats'] },
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
const KEEP: (keyof Settings)[] = ['catOverride', 'tipsSeen', 'recent', 'leftWidth', 'measureWidth', 'panelsClosed', 'panelLayout', 'layouts', 'workspaces', 'gapFactors', 'leftPanel', 'measureOpen', 'settingsAll', 'folder', 'exExpanded', 'session'];

export function resetSettings(): void {
  for (const key of Object.keys(DEFAULTS) as (keyof Settings)[]) {
    if (!KEEP.includes(key)) setSetting(key, structuredClone(DEFAULTS[key]) as never);
  }
}

/** Collapses or expands a sidebar panel (`explorer`, `measure.cur`, `measure.stats`). */
export function setPanelOpen(name: string, open: boolean): void {
  const rest = settings.panelsClosed.filter((n) => n !== name);
  setSetting('panelsClosed', open ? rest : [...rest, name]);
}
export const PANELS = ['explorer', 'settings', 'measure.cur', 'measure.stats'];
/** Collapses every sidebar panel, or (when all are collapsed) expands them. */
export function toggleAllPanels(): void {
  setSetting('panelsClosed', settings.panelsClosed.length >= PANELS.length ? [] : [...PANELS]);
}
/** Back to the default sidebar sizes, sides and panels. */
export function resetUiLayout(): void {
  setSetting('leftWidth', DEFAULTS.leftWidth);
  setSetting('measureWidth', DEFAULTS.measureWidth);
  setSetting('panelsClosed', []);
  setSetting('panelLayout', structuredClone(DEFAULTS.panelLayout));
}

/** Which sidebar a panel is in. */
export const sideOf = (id: PanelId): 'left' | 'right' => (settings.panelLayout.right.includes(id) ? 'right' : 'left');

/** Moves a panel to a sidebar, before panel `before` (or last), and shows it there. */
export function movePanel(id: PanelId, side: 'left' | 'right', before: PanelId | null = null): void {
  const L = settings.panelLayout;
  const left = L.left.filter((x) => x !== id);
  const right = L.right.filter((x) => x !== id);
  const list = side === 'left' ? left : right;
  const at = before ? list.indexOf(before) : -1;
  list.splice(at < 0 ? list.length : at, 0, id);
  setSetting('panelLayout', { left, right });
  // follow it, so it never seems to vanish
  if (side === 'left') setSetting('leftPanel', GROUP_OF[id]);
  else setSetting('measureOpen', true);
}

/** The rail group the left sidebar shows: the one chosen, else the first with panels there ('' when hidden). */
export function leftView(): LeftPanel {
  const has = (g: LeftPanel) => settings.panelLayout.left.some((id) => GROUP_OF[id] === g);
  if (!settings.leftPanel) return '';
  if (has(settings.leftPanel)) return settings.leftPanel;
  return settings.panelLayout.left.length ? GROUP_OF[settings.panelLayout.left[0]] : '';
}

/** The theme the page shows now (Match system resolved). */
export const shownTheme = (): 'light' | 'dark' =>
  settings.theme === 'system' ? (matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark') : settings.theme;

/** The theme on the page: Match system leaves it to the colour scheme. */
export function applyTheme(): void {
  const t = settings.theme;
  if (t === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = t;
}
