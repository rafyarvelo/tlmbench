// Settings: everything configurable, saved in the app's local storage as it changes (CLAUDE.md: production-like).
// Components and app.ts read `settings` directly; changes go through `setSetting` so they are saved and applied.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

import type { TimeFormat } from './axes';
import type { Limits } from './display.svelte';
import type { Pane } from './stack.svelte';
import type { Workspace } from './workspaces';
import type { CustomWidget } from './widgets';

export type Mode = 'box' | 'pan' | 'cursor';
export type Theme = 'system' | 'light' | 'dark';
export type Tooltips = 'short' | 'full' | 'off';
/** The panes a file opens with. */
export type StarterPanes = 'starter' | 'starterboth' | 'table' | 'values' | 'none';
export type TimeAxis = 'auto' | 'bottom' | 'every';
export type ZoomSpeed = 'slow' | 'normal' | 'fast';
/** What the left sidebar shows, a rail group (empty: hidden). */
export type LeftPanel = 'explorer' | 'events' | 'checks' | 'layouts' | 'settings' | 'measure' | '';
/** A sidebar panel: Explorer, Settings, and Measure's Cursors and Stats. */
export type PanelId = 'explorer' | 'events' | 'checks' | 'layouts' | 'settings' | 'cur' | 'stats';
/** The sidebars' panels, top to bottom (owner, Nov 7: they drag by their title to reorder or change sides). */
/** A category: where events are sorted, and the colour that marks them. */
export interface EventCategory {
  id: string;
  name: string;
  color: string;
  /** Its events are marked on charts (the panel's chips decide too). */
  chart: boolean;
}

export const DEFAULT_CATEGORIES: EventCategory[] = [
  { id: 'alarm', name: 'Alarm', color: '#FF5A6E', chart: true },
  { id: 'quality', name: 'Data quality', color: '#FFB547', chart: true },
  { id: 'signal', name: 'Signal loss', color: '#9D8CFF', chart: true },
  { id: 'mode', name: 'Mode change', color: '#F25FC0', chart: true },
  { id: 'milestone', name: 'Milestone', color: '#3FCF8E', chart: true },
  { id: 'check', name: 'Check failed', color: '#FF7A59', chart: true },
  { id: 'trigger', name: 'Trigger', color: '#E8C547', chart: true },
];

/** What the Connect dialog collects (see Connect.svelte). */
export interface ConnectForm {
  kind: 'playback' | 'serial' | 'udp' | 'tcp' | 'file';
  name: string;
  record: boolean;
  port: string;
  baud: string;
  address: string;
  path: string;
  descriptor: string;
  /** File playback: the recording, the speed, where to start (seconds) and whether it loops. */
  recording: string;
  speed: number;
  startAt: number;
  loop: boolean;
}
export interface SavedConnection extends ConnectForm {
  id: string;
}
/** A live trigger as the popover edits it (the core's `trigger::Spec`). */
export interface TriggerForm {
  channel: string;
  kind: 'rise' | 'fall' | 'above' | 'below';
  level: number;
  pre: number;
  post: number;
  repeat: boolean;
}

export interface PanelLayout {
  left: PanelId[];
  right: PanelId[];
}
/** Each panel's rail group: the left sidebar shows one group at a time, the right one all its panels. */
export const GROUP_OF: Record<PanelId, Exclude<LeftPanel, ''>> = { explorer: 'explorer', events: 'events', checks: 'checks', layouts: 'layouts', settings: 'settings', cur: 'measure', stats: 'measure' };
export const PANEL_TITLE: Record<PanelId, string> = { explorer: 'Explorer', events: 'Events', checks: 'Rules & checks', layouts: 'Layouts', settings: 'Settings', cur: 'Cursors', stats: 'Stats' };

/** A folder's session: the files open (in tab order), the one in front, the layouts shown in tabs, and each file's
 *  view and cursors (nanoseconds as strings). `at`: when it was last saved. */
export interface ExportOpts {
  format: 'csv' | 'parquet' | 'arrow' | 'hdf5' | 'mat';
  range: 'view' | 'cursors' | 'all';
  channels: 'tab' | 'all';
  layout: 'wide' | 'long';
  resample: boolean;
  hz: number;
  time: 'auto' | 'seconds' | 'iso' | 'ns';
  raw: boolean;
}
export interface ImageOpts {
  format: 'png' | 'svg';
  what: 'all' | 'sel';
  scale: 1 | 2;
}

export interface Session {
  files: string[];
  active: string | null;
  ws?: { path: string; name: string; at: number; shared?: string }[];
  activeWs?: string;
  views?: Record<string, { t0: string; t1: string; A?: string; B?: string }>;
  /** Tabs with several files, by the primary's path: the others in slot order with their alignment (the knots reproduce it exactly). */
  groups?: Record<string, { path: string; method: string; seconds: number; channel?: string; plugin?: string; params?: string; locked: boolean; knots: [string, string][]; pairs: [number, number][] }[]>;
  at?: number;
}

/** A window as it was when last seen: where it was, and for a window other than the first its folder and tabs. Reopening
 *  the app restores each one. */
export interface WindowSaved {
  folder: string;
  session?: Session;
  frame?: { x: number; y: number; w: number; h: number };
}

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
  /** A locked view still lets tables sort and filter (off: each table keeps its sort and filters). */
  lockSortFilter: boolean;
  /** A click on a table header sorts (off: it selects the column, and sorting is in the header's menu). */
  tableClickSort: boolean;
  /** The wheel zooms the charts (off: Ctrl+wheel zooms and the wheel pans). */
  wheelZoom: boolean;
  /** A channel dropped where a new pane goes makes a chart, or the same view as the pane it came from. */
  dropNewView: 'chart' | 'same';
  /** How-to notes in the panels. */
  showHints: boolean;
  overviewTop: boolean;
  leftPanel: LeftPanel;
  /** Sidebar widths in px (dragged at their inner edge), sidebar panels collapsed (by name), and Explorer and Measure traded sides. */
  /** This window follows the time view of the other windows that link too (one flight over two monitors), and sends its own. */
  linkView: boolean;
  /** And the cursors A and B. */
  linkCursors: boolean;
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
  /** A reduced trace (min and max per pixel column) gets a faint fill between its extremes. */
  envelope: boolean;
  /** The channel the overview is pinned to (null: it follows the selected pane). */
  overviewCh: number | null;
  /** Tooltips (Short retires each after three showings; `tipsSeen` counts them by tip). */
  tooltips: Tooltips;
  tipsSeen: Record<string, number>;
  /** The start page shows at launch, and the recordings opened last (newest first). */
  showStart: boolean;
  recent: string[];
  /** Where files go after the import dialog: each in a tab, or just into Explorer. */
  importWhere: 'tabs' | 'shared' | 'current' | 'none';
  /** Export data…: what was chosen last, and Save the view as an image…. */
  exportData: ExportOpts;
  exportImage: ImageOpts;
  /** Derived channels: the recipes added to one file (by path, in the order they were added), and the automatic ones left out of it. */
  fileRecipes: Record<string, string[]>;
  fileRecipesOff: Record<string, string[]>;
  /** What a file's channels are called when a tab holds several files (Channel prefix…), by path; none: its name. */
  filePrefix: Record<string, string>;
  /** Whether a file in a tab with several shows its short name (`AAPL · Close`) or its slot number (`Close·2`). */
  fileLabel: 'name' | 'number';
  /** How each channel looks, by channel name (in every file): alias, colour, decimals, unit, line style. */
  chdisp: Record<string, ChannelDisplay>;
  /** Warn and alarm limits by channel name; a channel without an entry has the starting ones for its name, if any. */
  limits: Record<string, Limits>;
  /** Widgets of your own (declarative SVG or code), and the shared folders whose code widgets you allowed to run. */
  customWidgets: CustomWidget[];
  trustedWidgetSources: string[];
  /** Play pauses at an event: 'off', 'any', or 'cat:<id>' for one category's events. */
  stopAt: string;
  /** Event markers show on charts and the overview (E); the Events panel's chips decide too unless unlinked. */
  eventsOn: boolean;
  evChartsFollow: boolean;
  /** Event categories (name, colour, whether charts mark them); events point at them by id. */
  eventCats: EventCategory[];
  /** Which check sets run where besides "every file": by file path and by layout name. */
  checkAttach: { files: Record<string, string[]>; layouts: Record<string, string[]> };
  /** Value labels (enums) by channel name: the stored number as text → what it is called. */
  valueMaps: Record<string, Record<string, string>>;
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
  /** Draw every sample up to this many in view per channel ('0': only when there are at most four per pixel). */
  drawRaw: string;
  /** What a line does at a gap (a time jump, a missing value, rows Apply to charts leaves out): break, or connect with a dashed segment. */
  gapLines: 'break' | 'connect';
  /** Settings shows every setting, not just the everyday ones. */
  settingsAll: boolean;
  /** The folder Explorer lists, and its folders (and open files) shown expanded. */
  folder: string;
  exExpanded: string[];
  /** The tabs open when the app last closed (their files, then layouts shown on them) and the one in front. */
  /** A session per folder ('' for none, owner Nov 7): its tabs, open files, and each file's time range and cursors,
   *  restored when the folder opens and listed under Recent sessions on the start page. */
  sessions: Record<string, Session>;
  /** The windows open, by name (`main`, `win-2`, …). */
  windows: Record<string, WindowSaved>;
  /** Where live sessions are recorded ('' is the default folder in the app's data folder). */
  recordTo: string;
  /** The native plugins turned on, by file name. They run with your rights, so each starts off. */
  nativePlugins: string[];
  /** Folders' shared plugins (`<folder>/.tlmbench/plugins`, by path) you trust to load; they stay listed, not loaded, until then. */
  trustedPluginFolders: string[];
  /** Connections saved from the Connect dialog, listed on the start page. */
  savedConnections: SavedConnection[];
  /** The Connect dialog's last form, and the trigger's last settings. */
  liveForm: Partial<ConnectForm>;
  liveTrigger: TriggerForm;
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
  lockSortFilter: true,
  tableClickSort: false,
  wheelZoom: true,
  dropNewView: 'chart',
  showHints: true,
  overviewTop: false,
  leftPanel: 'explorer',
  linkView: false,
  linkCursors: false,
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
  envelope: true,
  overviewCh: null,
  tooltips: 'short',
  tipsSeen: {},
  showStart: true,
  recent: [],
  importWhere: 'tabs',
  exportData: { format: 'csv', range: 'view', channels: 'tab', layout: 'wide', resample: false, hz: 100, time: 'auto', raw: false },
  exportImage: { format: 'png', what: 'all', scale: 2 },
  fileRecipes: {},
  fileRecipesOff: {},
  filePrefix: {},
  fileLabel: 'name',
  chdisp: {},
  limits: {},
  customWidgets: [],
  trustedWidgetSources: [],
  valueMaps: {},
  stopAt: 'off',
  eventsOn: true,
  evChartsFollow: true,
  eventCats: structuredClone(DEFAULT_CATEGORIES),
  checkAttach: { files: {}, layouts: {} },
  exCategories: true,
  exCatClosed: [],
  catOverride: {},
  exFilesShut: [],
  exShowAll: false,
  exOnlyOpen: false,
  panelLayout: { left: ['explorer', 'events', 'checks', 'layouts', 'settings'], right: ['cur', 'stats'] },
  measureOpen: false,
  measureAuto: true,
  showPerf: false,
  drawRaw: '0',
  gapLines: 'break',
  settingsAll: false,
  folder: '',
  exExpanded: [],
  sessions: {},
  windows: {},
  recordTo: '',
  nativePlugins: [],
  trustedPluginFolders: [],
  savedConnections: [],
  liveForm: {},
  liveTrigger: { channel: '', kind: 'rise', level: 0, pre: 2, post: 3, repeat: false },
  layouts: {},
  workspaces: [],
  gapFactors: {},
};

const KEY = 'tlmbench.settings';

/** The shared settings as last saved or received (to send only real changes). */
let sharedText = '';

/** What each window keeps for itself (layout, not settings): the sidebars shown, their widths and the Explorer's state.
 *  Everything else is shared by every window and applies to all of them at once. */
export const WINDOW_KEYS: (keyof Settings)[] = ['linkView', 'linkCursors', 'leftWidth', 'measureWidth', 'panelsClosed', 'panelLayout', 'leftPanel', 'measureOpen', 'exExpanded'];

/** This window's name (`main`, then `win-2`, …). */
function windowLabel(): string {
  try {
    return getCurrentWindow().label;
  } catch {
    return 'main';
  }
}
export const WINDOW = windowLabel();
const WINDOW_KEY = `${KEY}.window.${WINDOW}`;

function parse(text: string | null): Record<string, unknown> {
  try {
    return JSON.parse(text ?? '{}');
  } catch {
    return {};
  }
}

function load(): Settings {
  try {
    const s: Settings = { ...structuredClone(DEFAULTS), ...parse(localStorage.getItem(KEY)), ...parse(localStorage.getItem(WINDOW_KEY)) };
    // settings saved before a panel existed: it joins the left sidebar, after Explorer
    const L = s.panelLayout;
    if (L && !L.left.includes('layouts') && !L.right.includes('layouts')) L.left.splice(Math.min(1, L.left.length), 0, 'layouts');
    if (L && !L.left.includes('events') && !L.right.includes('events')) L.left.splice(Math.min(1, L.left.length), 0, 'events');
    if (L && !L.left.includes('checks') && !L.right.includes('checks')) L.left.splice(Math.min(2, L.left.length), 0, 'checks');
    // the category check failures sort into joins the ones saved before it
    for (const id of ['check', 'trigger']) if (!s.eventCats.some((c) => c.id === id)) s.eventCats.push(structuredClone(DEFAULT_CATEGORIES.find((c) => c.id === id)!));
    return s;
  } catch {
    return structuredClone(DEFAULTS);
  }
}

export const settings: Settings = $state(load());
sharedText = JSON.stringify(sharedPart(structuredClone($state.snapshot(settings)) as unknown as Record<string, unknown>));

const listeners = new Set<(key: keyof Settings) => void>();

/** Calls `f` with each setting's name after it changes. */
export function onSetting(f: (key: keyof Settings) => void): void {
  listeners.add(f);
}

/** What a channel shows differently from the recording: its name, colour (a palette slot 1–8 or `#rrggbb`), decimals,
 *  unit label and line style. Anything left out is the default. */
export interface ChannelDisplay {
  alias?: string;
  color?: number | string;
  dec?: number;
  unit?: string;
  width?: number;
  dash?: boolean;
  step?: boolean;
  /** Raw counts to engineering units: shown = stored × scale + offset (on top of what the format says). */
  scale?: number;
  offset?: number;
  /** Which samples are missing: stored values from a list, outside a valid range, or while a condition holds. */
  missing?: { vals?: number[]; lo?: number; hi?: number; when?: string };
}

function sharedPart(all: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(all)) if (!(WINDOW_KEYS as string[]).includes(k)) out[k] = all[k];
  return out;
}

export function saveSettings(): void {
  const all = $state.snapshot(settings) as unknown as Record<string, unknown>;
  const shared = JSON.stringify(sharedPart(all));
  try {
    localStorage.setItem(KEY, shared);
    localStorage.setItem(WINDOW_KEY, JSON.stringify(Object.fromEntries(WINDOW_KEYS.map((k) => [k, all[k]]))));
  } catch {
    /* private mode: settings last this session */
  }
  if (shared !== sharedText) {
    sharedText = shared;
    // the core keeps them for the next start and tells the other windows
    invoke('settings_save', { text: shared }).catch(() => {});
  }
}

/** Applies settings another window saved. */
export function applyShared(text: string): void {
  if (text === sharedText) return;
  sharedText = text;
  const incoming = parse(text);
  try {
    localStorage.setItem(KEY, text);
  } catch {
    /* private mode */
  }
  for (const key of Object.keys(incoming) as (keyof Settings)[]) {
    if ((WINDOW_KEYS as string[]).includes(key)) continue;
    if (JSON.stringify($state.snapshot(settings[key])) === JSON.stringify(incoming[key])) continue;
    (settings as unknown as Record<string, unknown>)[key] = incoming[key];
    listeners.forEach((f) => f(key));
  }
}

/** Follows the other windows' changes to the shared settings. */
export function followOtherWindows(): void {
  listen<{ from: string; text: string }>('settings-changed', (e) => {
    if (e.payload.from !== WINDOW) applyShared(e.payload.text);
  }).catch(() => {});
}

export function setSetting<K extends keyof Settings>(key: K, value: Settings[K]): void {
  settings[key] = value;
  saveSettings();
  listeners.forEach((f) => f(key));
}

/** What Reset to defaults keeps: the panes and gap factors remembered per file, and where the sidebars are. */
const KEEP: (keyof Settings)[] = ['linkView', 'linkCursors', 'catOverride', 'tipsSeen', 'recent', 'importWhere', 'exportData', 'exportImage', 'fileRecipes', 'fileRecipesOff', 'filePrefix', 'chdisp', 'limits', 'customWidgets', 'trustedWidgetSources', 'trustedPluginFolders', 'valueMaps', 'eventCats', 'checkAttach', 'leftWidth', 'measureWidth', 'panelsClosed', 'panelLayout', 'layouts', 'workspaces', 'gapFactors', 'leftPanel', 'measureOpen', 'settingsAll', 'folder', 'exExpanded', 'sessions', 'windows', 'recordTo', 'savedConnections', 'liveForm', 'liveTrigger'];

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
export const PANELS = ['explorer', 'layouts', 'settings', 'measure.cur', 'measure.stats'];
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
