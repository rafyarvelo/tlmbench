import type { CustomWidget } from './widgets';

// Shared configuration in the UI: the Share dialog's state (what you have against the folder's `.tlmbench/`), and what
// the app knows about the folder it reads configuration from. The core compares and writes; the dialog only picks.

/** The kinds of object a `.tlmbench/` holds, in the order the dialog lists them. */
export const KINDS = ['views', 'events', 'checks', 'categories', 'recipes', 'profiles', 'decoders', 'widgets', 'icons', 'timemaps'] as const;
export type Kind = (typeof KINDS)[number];

export const KIND_TITLE: Record<Kind, string> = {
  views: 'Layouts',
  events: 'Events you added',
  checks: 'Rules & check sets',
  categories: 'Event categories',
  recipes: 'Recipes',
  profiles: 'Import profiles',
  decoders: 'Decoders',
  widgets: 'Widgets',
  icons: 'Icons',
  timemaps: 'Time maps (alignment)',
};

/** One object: yours only (new), both but different (changed), equal (same), or only the folder's (folder). */
export interface ShareRow {
  kind: Kind;
  key: string;
  name: string;
  state: 'new' | 'changed' | 'same' | 'folder';
  detail: string | null;
}

/** What to do with a row: write yours to the folder, take the folder's into the app, or leave it. */
export type Pick = 'share' | 'use' | '';

export interface Problem {
  path: string;
  why: string;
}

export const share = $state({
  open: false,
  busy: false,
  error: '',
  rows: [] as ShareRow[],
  problems: [] as Problem[],
  pick: {} as Record<string, Pick>,
  /** The folder looks writable; if not, the dialog offers Export… instead of Share. */
  writable: true,
  /** The folder is a git repository: offer a .gitignore line for the recordings. */
  git: false,
  ignore: true,
  /** What the last Apply did. */
  done: '',
  showSame: false,
});

/** The folder the app reads shared configuration from (the nearest `.tlmbench/`), as `config_use` last reported. */
export const cfg = $state({
  dir: null as string | null,
  /** Names of the recipes and check sets the folder holds, so lists can mark them. */
  recipes: [] as string[],
  checks: [] as string[],
  problems: [] as Problem[],
  /** The folder's event categories (they win for shared rules and events) and widgets. */
  categories: [] as { id: string; name: string; color: string; show_on_charts: boolean }[],
  widgets: [] as CustomWidget[],
});

export const rowKey = (r: { kind: string; key: string }): string => `${r.kind}:${r.key}`;

/** What a row does by default: yours that the folder lacks are shared, the folder's that you lack are taken, and what
 * differs is left until a person says which side is right (nothing silently overwrites a colleague's file). */
export const defaultPick = (r: ShareRow, writable: boolean): Pick => (r.state === 'new' ? (writable ? 'share' : '') : r.state === 'folder' ? 'use' : '');

/** The choices a row offers. */
export function choices(r: ShareRow, writable: boolean): { value: Pick; label: string }[] {
  const skip = { value: '' as Pick, label: 'Leave' };
  if (r.state === 'new') return writable ? [{ value: 'share', label: 'Share' }, skip] : [skip];
  if (r.state === 'folder') return [{ value: 'use', label: 'Use the folder’s' }, skip];
  if (r.state === 'changed') return [...(writable ? [{ value: 'share' as Pick, label: r.kind === 'events' ? 'Share (merges)' : 'Share mine' }] : []), { value: 'use', label: 'Use the folder’s' }, skip];
  return [skip];
}
