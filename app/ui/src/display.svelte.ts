// How channels look: the alias, colour, decimals and unit label a person set (settings.chdisp, by channel name), over
// what the recording says. Everything that draws a channel's name, colour, unit or value asks here.

import { settings, setSetting, type ChannelDisplay } from './settings.svelte';
import { colorIdx } from './stack.svelte';
import { ui } from './ui.svelte';

/** The display settings of channel `name`. */
export const dispOf = (name: string): ChannelDisplay => settings.chdisp[name] ?? {};

const info = (ch: number) => ui.channels[ch];

/** The name shown for channel `ch`: its alias, else its own name. */
export const chName = (ch: number): string => {
  const c = info(ch);
  return c ? dispOf(c.name).alias || c.name : String(ch);
};

/** The unit shown for channel `ch`: the label set for it, else the recording's. */
export const chUnit = (ch: number): string => {
  const c = info(ch);
  return c ? (dispOf(c.name).unit ?? c.unit ?? '') : '';
};

/** A colour set for channel `ch` as a CSS colour, or null for the automatic palette slot. */
export function chColor(ch: number): string | null {
  const c = info(ch);
  const v = c ? dispOf(c.name).color : undefined;
  return typeof v === 'number' ? `var(--trace-${v})` : (v ?? null);
}

/** Changes channel `name`'s display; empty values go back to the default, and nothing left removes the entry. */
export function setDisplay(name: string, d: ChannelDisplay): void {
  const clean: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(d)) if (v !== '' && v != null && v !== false) clean[k] = v;
  const all = { ...settings.chdisp };
  if (Object.keys(clean).length) all[name] = clean as ChannelDisplay;
  else delete all[name];
  setSetting('chdisp', all);
}

/** The CSS colour channel `ch` is drawn in: the one set for it, else its palette slot. */
export const chCss = (ch: number): string => chColor(ch) ?? `var(--trace-${colorIdx(ch) + 1})`;

/** The channel editor: which channel of the open file it shows (null: closed). */
export const editor = $state({ ch: null as number | null, error: '', tab: 'display' as 'display' | 'limits' | 'labels' });
export const openChannelEditor = (ch: number, tab: 'display' | 'limits' | 'labels' = 'display'): void => void Object.assign(editor, { ch, tab, error: '' });
export const closeChannelEditor = (): void => void (editor.ch = null);

// ---------- limits ----------
// Warn and alarm limits, low and high, by channel name (so a limit applies in every pane and file). Common channels
// start with sensible ones; once a channel has an entry of its own, even an empty one, the starting ones are gone.

/** Alarm at or below / above, warn at or below / above. */
export interface Limits {
  al?: number;
  ah?: number;
  wl?: number;
  wh?: number;
}
export type Level = '' | 'ok' | 'warn' | 'alarm';

const SEEDS: [RegExp, Limits][] = [
  [/^battery_v$/, { wl: 22.2, al: 21.0 }],
  [/^cell_\d_v$/, { wl: 3.65, al: 3.5 }],
  [/^esc_temp_\d$/, { wh: 60, ah: 75 }],
  [/^current_a$/, { wh: 30, ah: 38 }],
  [/^motor(_\d)?_rpm$/, { wh: 7000, ah: 7600 }],
  [/^gps_sats$/, { wl: 8, al: 6 }],
  [/^gps_hdop$/, { wh: 1.5, ah: 2.5 }],
  [/^(roll|pitch)$/, { wl: -20, wh: 20, al: -35, ah: 35 }],
  [/^failsafe$/, { ah: 0.5 }],
];

/** The limits of channel `name`: its own, else the starting ones for its name. */
export function limitsOf(name: string): Limits {
  if (Object.hasOwn(settings.limits, name)) return settings.limits[name];
  return { ...(SEEDS.find(([re]) => re.test(name))?.[1] ?? {}) };
}

/** Sets channel `name`'s limits (empty keeps "no limits" rather than bringing the starting ones back). */
export function setLimits(name: string, lim: Limits): void {
  setSetting('limits', { ...settings.limits, [name]: lim });
}

export const hasLimits = (lim: Limits): boolean => Object.values(lim).some((x) => x != null);

/** Where `v` stands against `lim`: alarm, warn, ok (inside limits), or '' (no limits or no value). */
export function levelOf(lim: Limits, v: number | null | undefined): Level {
  if (v == null || !Number.isFinite(v)) return '';
  if ((lim.al != null && v <= lim.al) || (lim.ah != null && v >= lim.ah)) return 'alarm';
  if ((lim.wl != null && v <= lim.wl) || (lim.wh != null && v >= lim.wh)) return 'warn';
  return hasLimits(lim) ? 'ok' : '';
}

export const limitText = (lim: Limits): string =>
  [lim.al != null ? `alarm ≤ ${lim.al}` : '', lim.wl != null ? `warn ≤ ${lim.wl}` : '', lim.wh != null ? `warn ≥ ${lim.wh}` : '', lim.ah != null ? `alarm ≥ ${lim.ah}` : '']
    .filter(Boolean)
    .join(' · ');

/** The limits of channel `ch` of the open file. */
export const chLimits = (ch: number): Limits => limitsOf(ui.channels[ch]?.name ?? '');
/** Where value `v` of channel `ch` stands against its limits. */
export const chLevel = (ch: number, v: number | null | undefined): Level => levelOf(chLimits(ch), v);

/** The display settings of channel `ch` of the open file. */
export const chDisp = (ch: number): ChannelDisplay => dispOf(ui.channels[ch]?.name ?? '');
/** Line width factors by the editor's setting (1 thin, 2 thick, 3 heavy; normal is 1). */
export const WIDTHS: Record<number, number> = { 1: 0.7, 2: 1.8, 3: 2.8 };

/** What the core applies to channels' stored values, from the display settings: scale and offset, missing values. */
export interface AdjustOut {
  channel: string;
  scale?: number;
  offset?: number;
  values?: number[];
  lo?: number;
  hi?: number;
  when?: string;
}
export function adjustList(): AdjustOut[] {
  const out: AdjustOut[] = [];
  for (const [channel, d] of Object.entries(settings.chdisp)) {
    const m = d.missing;
    if ((d.scale ?? 1) === 1 && !d.offset && !m) continue;
    out.push({ channel, scale: d.scale, offset: d.offset, values: m?.vals, lo: m?.lo, hi: m?.hi, when: m?.when });
  }
  return out;
}
