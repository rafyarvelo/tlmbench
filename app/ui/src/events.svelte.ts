// Events: what you add (markers and ranges) to a recording, kept by the core per recording in the app cache, and the
// categories they sort into (settings). The Events panel filters and groups them; charts, the overview and tables
// read `events.list` (the ones the category filters show) to mark them.

import { type EventCategory, settings } from './settings.svelte';
import { cfg } from './share.svelte';

export { DEFAULT_CATEGORIES, type EventCategory } from './settings.svelte';

/** An event as the core keeps it: times are decimal strings (exact i64 nanoseconds). */
export interface TlmEvent {
  id: string;
  name: string;
  start: string;
  duration: string;
  category: string;
  source: string;
  /** Who added it (the account name), when known. */
  by?: string;
  channels: string[];
}

export const UNCATEGORIZED = '#8A94A3';

export const events = $state({
  /** Every event of the open file, in start order: the ones you added, and the read-only ones from rules and checks. */
  list: [] as TlmEvent[],
  mine: [] as TlmEvent[],
  auto: [] as TlmEvent[],
  /** Rules and checks are still running on the open file. */
  checking: false,
  /** The panel: text filter, grouping, categories hidden in the list (ids; '' is uncategorized), groups closed, the event being edited. */
  q: '',
  group: '' as '' | 'cat' | 'name' | 'channel' | 'source',
  hidden: [] as string[],
  closed: [] as string[],
  editing: null as string | null,
  /** A line under the panel's buttons ("Place cursor A first"). */
  msg: '',
});

const byStart = (a: TlmEvent, b: TlmEvent) => (BigInt(a.start) < BigInt(b.start) ? -1 : BigInt(a.start) > BigInt(b.start) ? 1 : 0);
/** The events you added, as the core keeps them. */
export function setMine(list: TlmEvent[]): void {
  events.mine = list;
  events.list = [...list, ...events.auto].sort(byStart);
}
/** The events rules and checks make, read-only: recomputed whenever they run. */
export function setAuto(list: TlmEvent[]): void {
  events.auto = list;
  events.list = [...events.mine, ...list].sort(byStart);
}

/** A category by id: the folder's win for shared rules and events, so a shared view looks the same for everyone. */
export const catOf = (id: string): EventCategory | undefined => {
  const f = cfg.categories.find((c) => c.id === id);
  return f ? { id: f.id, name: f.name, color: f.color, chart: f.show_on_charts } : settings.eventCats.find((c) => c.id === id);
};
export const catName = (id: string): string => catOf(id)?.name ?? 'Uncategorized';
export const catColor = (id: string): string => catOf(id)?.color ?? UNCATEGORIZED;

/** What the panel's filters leave in the list. */
export function listed(): TlmEvent[] {
  const q = events.q.trim().toLowerCase();
  return events.list.filter(
    (e) => !events.hidden.includes(e.category) && (!q || e.name.toLowerCase().includes(q) || catName(e.category).toLowerCase().includes(q) || e.channels.some((c) => c.toLowerCase().includes(q))),
  );
}

/** A fresh id for an event. */
export const newId = (): string => 'm' + Date.now().toString(36) + Math.floor(Math.random() * 1296).toString(36);

/** The events charts and the overview mark: every event unless E is off, in categories shown on charts (and, while the
 *  panel's chips are linked, not hidden there). */
export function marked(): TlmEvent[] {
  if (!settings.eventsOn) return [];
  return events.list.filter((e) => (catOf(e.category)?.chart ?? true) && !(settings.evChartsFollow && events.hidden.includes(e.category)));
}

// What charts and the overview mark follows the list and the panel's chips: draw again when either changes.
let redraw = () => {};
export const onMarksChange = (f: () => void): void => void (redraw = f);
$effect.root(() => {
  $effect(() => {
    void JSON.stringify([events.list, events.hidden]);
    redraw();
  });
});
