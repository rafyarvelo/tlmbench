// The command table: every command the native menu bar has, with its menu, its label and its shortcut, in one place.
// The menu is built from this table (the window sends it to the core at start, `menu_set`), and the keyboard handler in
// app.ts reads the same table, so a menu item and its shortcut can't drift apart. What a command does is in `RUN` (app.ts);
// a test checks that every entry has a menu item and an action.
//
// The rule for shortcuts, for the whole table:
//  - `accel` is a combination with ⌘ (Ctrl on Windows and Linux) or ⌥. The menu binds it, so macOS sends the key to the menu
//    and shows it beside the item. The keydown handler runs the same command for a browser tab (no menu there) and where
//    the webview sees the key first; a menu click right after the same key press is ignored, so it never runs twice.
//  - `key` is a single key (E, L, T, F, Esc, Backspace). It is shown in the label, never bound by the menu: it would stop
//    that key typing in every text field. The keydown handler owns it.
//  - `text` marks a command a focused text field keeps for itself (⌘Z is the field's undo there): the menu item then sends
//    the field's own action, and the keyboard handler leaves the key alone.

export type MenuName = 'File' | 'Edit' | 'View' | 'Pane' | 'Window' | 'Help';

export interface Command {
  id: string;
  menu: MenuName;
  label: string;
  /** Tauri accelerator syntax: `CmdOrCtrl+Shift+N`. Always has a modifier. */
  accel?: string;
  /** A single-key shortcut shown after the label; the menu doesn't bind it. */
  key?: string;
  /** The item has a check mark (a setting that is on or off). */
  check?: boolean;
  /** A separator goes before this item. */
  sep?: boolean;
  /** A focused text field keeps this shortcut. */
  text?: boolean;
}

export const COMMANDS: Command[] = [
  { id: 'open', menu: 'File', label: 'Open recording…', accel: 'CmdOrCtrl+O' },
  { id: 'openFolder', menu: 'File', label: 'Open folder…', accel: 'CmdOrCtrl+Shift+O' },
  { id: 'saveLayout', menu: 'File', label: 'Save layout', accel: 'CmdOrCtrl+S', sep: true },
  { id: 'exportData', menu: 'File', label: 'Export data…', sep: true },
  { id: 'exportHtml', menu: 'File', label: 'Export to HTML…' },
  { id: 'saveImage', menu: 'File', label: 'Save the view as an image…' },
  { id: 'closeTab', menu: 'File', label: 'Close tab', sep: true },

  { id: 'undo', menu: 'Edit', label: 'Undo', accel: 'CmdOrCtrl+Z', text: true },
  { id: 'redo', menu: 'Edit', label: 'Redo', accel: 'CmdOrCtrl+Shift+Z', text: true },
  { id: 'clearCursors', menu: 'Edit', label: 'Clear cursors', key: 'Esc', sep: true },

  { id: 'back', menu: 'View', label: 'Back to the previous view', key: 'Backspace' },
  { id: 'fit', menu: 'View', label: 'Fit to the data', key: 'F' },
  { id: 'zoomIn', menu: 'View', label: 'Zoom in', key: '+' },
  { id: 'zoomOut', menu: 'View', label: 'Zoom out', key: '−' },
  { id: 'events', menu: 'View', label: 'Event markers', key: 'E', check: true, sep: true },
  { id: 'lock', menu: 'View', label: 'Lock the time view', key: 'L', check: true },
  { id: 'overviewTop', menu: 'View', label: 'Overview above the panes', check: true },
  { id: 'theme', menu: 'View', label: 'Switch theme', key: 'T', sep: true },
  { id: 'settings', menu: 'View', label: 'Settings…', accel: 'CmdOrCtrl+,' },

  { id: 'newPane', menu: 'Pane', label: 'New pane', accel: 'CmdOrCtrl+Shift+N' },
  { id: 'duplicate', menu: 'Pane', label: 'Duplicate pane', accel: 'CmdOrCtrl+D' },
  { id: 'duplicateBeside', menu: 'Pane', label: 'Duplicate pane beside', accel: 'CmdOrCtrl+Shift+D' },
  { id: 'removeChannels', menu: 'Pane', label: 'Remove every channel', sep: true },
  { id: 'removePanes', menu: 'Pane', label: 'Remove every pane' },

  { id: 'focus', menu: 'Window', label: 'Focus mode', accel: 'CmdOrCtrl+.' },
  { id: 'panels', menu: 'Window', label: 'Collapse or expand the sidebar panels' },
  { id: 'perf', menu: 'Window', label: 'Show performance details', check: true },

  { id: 'tipsAgain', menu: 'Help', label: 'Show the tips again' },
];

const MAC = typeof navigator !== 'undefined' && /Mac/.test(navigator.platform);

/**
 * Shortcuts as people read them on their platform. The app writes them the Mac way (⌘S, ⌥⌘B, ⌘⇧N, ⌥←); on Windows and
 * Linux each becomes Ctrl+S, Ctrl+Alt+B, Ctrl+Shift+N, Alt+←. Tooltips, menus and hints all pass through here.
 */
export function keyName(text: string, mac = MAC): string {
  if (mac) return text;
  return text.replace(/([⌃⌥⇧⌘]+)([A-Za-z0-9]+|\S)/g, (_, mods: string, k: string) => {
    const m: string[] = [];
    if (mods.includes('⌘') || mods.includes('⌃')) m.push('Ctrl');
    if (mods.includes('⌥')) m.push('Alt');
    if (mods.includes('⇧')) m.push('Shift');
    return [...m, k].join('+');
  });
}

/** The label as the menu shows it: a single-key shortcut goes in brackets after it. */
export const menuLabel = (c: Command): string => (c.key ? `${c.label} (${c.key})` : c.label);

/** Whether a key press is this command's accelerator (⌘ on the Mac, Ctrl elsewhere; ⌥ and ⇧ as named). */
export function matches(c: Command, e: { key: string; metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean }): boolean {
  if (!c.accel) return false;
  const parts = c.accel.split('+');
  const key = parts[parts.length - 1].toLowerCase();
  const mods = new Set(parts.slice(0, -1));
  const mac = /Mac/.test(navigator.platform);
  const cmd = mods.has('CmdOrCtrl') || mods.has('Cmd') || mods.has('Ctrl');
  const primary = mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
  return e.key.toLowerCase() === key && (cmd ? primary : !(e.metaKey || e.ctrlKey)) && e.altKey === mods.has('Alt') && e.shiftKey === mods.has('Shift');
}
