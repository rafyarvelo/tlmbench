// Explorer's channel categories: by default the start of a channel's name (before the first `_` or `.`) when two or
// more share it, else "other"; a channel can be moved to a category of your own (kept per file, by channel name).

import { settings } from './settings.svelte';

export interface Group {
  name: string;
  chs: number[];
}

const prefix = (n: string) => n.split(/[_.]/)[0];
export const overrideKey = (path: string, name: string) => `${path}|${name}`;

/** The categories of a file's channels, in order ("other" last), with your moves applied. */
export function groupChannels(path: string, channels: { name: string }[]): Group[] {
  const count = new Map<string, number>();
  channels.forEach((c) => count.set(prefix(c.name), (count.get(prefix(c.name)) ?? 0) + 1));
  const by = new Map<string, number[]>();
  channels.forEach((c, ch) => {
    const g = settings.catOverride[overrideKey(path, c.name)] ?? ((count.get(prefix(c.name)) ?? 0) > 1 ? prefix(c.name) : 'other');
    by.set(g, [...(by.get(g) ?? []), ch]);
  });
  return [...by].map(([name, chs]) => ({ name, chs })).sort((a, b) => (a.name === 'other' ? 1 : b.name === 'other' ? -1 : 0));
}
