// Rules & checks: the sets the core keeps, which of them run on the open file, and what they found. A check says what
// should hold ("expect X when S") and gets a verdict; a rule marks events where its condition holds. The core runs
// them and caches the results by the file's contents and the set's version; here is what the panel, the badges, the
// chips and the events read. Results are kept by file path, so a tab that isn't in front keeps its badge.

import { settings } from './settings.svelte';

export type Verdict = 'fail' | 'warn' | 'err' | 'cant' | 'pass' | 'na' | 'mark' | 'off';

/** One row of a set, as the core keeps it (`check` is the core's name for the rows). */
export interface Check {
  name: string;
  kind: 'check' | 'rule';
  group: string;
  expect: string;
  when?: string;
  severity: 'fail' | 'warn';
  category?: string;
  grace: number;
  settle: number;
  enabled: boolean;
}

export interface CheckSet {
  schema: number;
  name: string;
  runs_on_any_file: boolean;
  info_groups: string[];
  check: Check[];
}

export interface Span {
  start: string;
  end: string;
}

export interface Outcome {
  name: string;
  verdict: Verdict;
  count: number;
  secs: number;
  runs: Span[];
  worst?: number;
  worst_t?: string;
  value?: number;
  function?: string;
  limit?: number;
  scope_secs?: number;
  message?: string;
  channels: string[];
}

export interface GroupRoll {
  name: string;
  verdict: Verdict;
  events: number;
  info: boolean;
  total: number;
  failed: number;
  warned: number;
  passed: number;
}

export interface Rollup {
  groups: GroupRoll[];
  verdict: Verdict;
  failed: number;
  warned: number;
  events: number;
  applies: boolean;
}

export interface SetResult {
  set: string;
  outcomes: Outcome[];
  rollup: Rollup;
}

export const RANK: Record<Verdict, number> = { fail: 5, warn: 4, err: 3, cant: 2, pass: 1, na: 0, mark: 0, off: 0 };
export const WORD: Record<Verdict, string> = { fail: 'Fail', warn: 'Warn', err: 'Error', cant: "Can't run", pass: 'Pass', na: 'Not applicable', mark: 'Marks events', off: 'Off' };
export const ICONCH: Record<Verdict, string> = { fail: '✗', warn: '!', err: '?', cant: '–', pass: '✓', na: '·', mark: '⚑', off: '○' };

export const checks = $state({
  /** The sets the core keeps. */
  sets: [] as CheckSet[],
  /** Results by file path, then set name. */
  runs: {} as Record<string, Record<string, SetResult>>,
  /** Running on the open file (the panel says "Checking…"). */
  running: false,
  /** Sets and groups the panel shows closed (`set` or `set/group`). */
  closed: [] as string[],
  /** The selected row (`set/index`). */
  sel: null as string | null,
  /** The editor: the set being edited (a copy), its original name, and the row to focus. */
  editor: null as null | { set: CheckSet; was: string | null; focus: number; cats: boolean },
  /** The folder run's dialog. */
  folder: false,
  /** A folder run's grid cell that was clicked: once its file is open and has run, go to the first violation in that group. */
  goto: null as null | { path: string; set: string; group: string },
});

/** A folder run: one set over the files of a folder. Each row is a file's result (or why it couldn't be checked). */
export const folderRun = $state({
  set: '',
  files: [] as string[],
  busy: false,
  done: 0,
  rows: {} as Record<string, { error?: string; res?: SetResult; first?: string }>,
  tab: 'grid' as 'grid' | 'report',
  group: '',
});

/** The sets that run on a file: the ones that run on any file, the ones attached to the file, to the layout in front
 *  and to the file's import profile. */
export function setsFor(path: string, layout: string | null, profile: string[] = []): CheckSet[] {
  const A = settings.checkAttach;
  const names = new Set([...(A.files[path] ?? []), ...(layout ? (A.layouts[layout] ?? []) : []), ...profile]);
  return checks.sets.filter((s) => s.runs_on_any_file || names.has(s.name));
}

export const resultsOf = (path: string): SetResult[] => Object.values(checks.runs[path] ?? {}).filter((r) => r.rollup.applies || r.rollup.events > 0 || r.rollup.verdict === 'mark');

/** What a file's badge says: checks (not rules) that failed and warned, over the sets that ran on it. */
export function summary(path: string): { failed: number; warned: number; passed: number; groups: string[]; sets: number } | null {
  const rs = resultsOf(path);
  if (!rs.length) return null;
  const sum = { failed: 0, warned: 0, passed: 0, groups: [] as string[], sets: 0 };
  for (const r of rs) {
    const set = checks.sets.find((s) => s.name === r.set);
    if (!set) continue;
    let counted = false;
    for (const g of r.rollup.groups) {
      if (g.info) continue;
      sum.failed += g.failed;
      sum.warned += g.warned;
      sum.passed += g.passed;
      counted ||= g.total > 0;
      if (g.failed || g.warned) sum.groups.push(`${r.set} · ${g.name}`);
    }
    if (counted) sum.sets++;
  }
  return sum.sets ? sum : null;
}

/** Channels used by a failing (or warning) check on a file, with the worst verdict. */
export function failingChannels(path: string): Map<string, Verdict> {
  const out = new Map<string, Verdict>();
  for (const r of Object.values(checks.runs[path] ?? {})) {
    const set = checks.sets.find((s) => s.name === r.set);
    if (!set) continue;
    set.check.forEach((k, i) => {
      const o = r.outcomes[i];
      if (k.kind !== 'check' || !o || (o.verdict !== 'fail' && o.verdict !== 'warn')) return;
      for (const c of o.channels) if (!out.has(c) || RANK[o.verdict] > RANK[out.get(c)!]) out.set(c, o.verdict);
    });
  }
  return out;
}

/** The violations of failing checks that read `channel` and cover `t` (ns): the gauges' "✗ check". */
export function violationAt(path: string, channel: string, t: bigint): { set: string; check: string } | null {
  for (const r of Object.values(checks.runs[path] ?? {})) {
    const set = checks.sets.find((s) => s.name === r.set);
    if (!set) continue;
    for (let i = 0; i < set.check.length; i++) {
      const o = r.outcomes[i];
      if (set.check[i].kind !== 'check' || !o || (o.verdict !== 'fail' && o.verdict !== 'warn') || !o.channels.includes(channel)) continue;
      if (o.runs.some((s) => t >= BigInt(s.start) && t <= BigInt(s.end))) return { set: r.set, check: set.check[i].name };
    }
  }
  return null;
}

/** A new row, empty. */
export const newCheck = (rule: boolean, group = 'Checks'): Check => ({ name: rule ? 'New rule' : 'New check', kind: rule ? 'rule' : 'check', group, expect: '', severity: 'fail', category: rule ? 'alarm' : undefined, grace: 0, settle: 0, enabled: true });

export const EVENT_RULES = 'Event rules';
export const newSet = (name: string): CheckSet => ({ schema: 1, name, runs_on_any_file: false, info_groups: [], check: [] });

/** The wording of one result: "Fail · 3× · worst 7995 at …", "max 16.2 (limit 20)", or why it didn't run. */
export function outcomeText(o: Outcome, fmtTime: (ns: string) => string, fmtSecs: (s: number) => string): string {
  const num = (x: number) => (Math.abs(x) >= 1000 || Number.isInteger(x) ? x.toFixed(0) : String(+x.toFixed(3)));
  if ((o.verdict === 'fail' || o.verdict === 'warn' || o.verdict === 'mark') && o.count > 0) {
    const p = [`${o.count}×`, fmtSecs(o.secs)];
    if (o.worst != null && o.worst_t) p.push(`worst ${num(o.worst)} at ${fmtTime(o.worst_t)}`);
    if (o.runs[0]) p.push(`first at ${fmtTime(o.runs[0].start)}`);
    return p.join(' · ');
  }
  if (o.value != null && o.function) return `${o.function} ${num(o.value)}${o.limit != null ? ` (limit ${num(o.limit)})` : ''}`;
  if (o.verdict === 'mark') return 'no events';
  return o.message ?? '';
}
