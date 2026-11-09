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
export const editor = $state({ ch: null as number | null });
export const openChannelEditor = (ch: number): void => void (editor.ch = ch);
export const closeChannelEditor = (): void => void (editor.ch = null);
