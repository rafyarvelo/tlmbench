// The CSV import dialog's state: what the core found in each file (`import_inspect`), the choices made about its
// time, rate and columns, and the profile those choices make (`profile_save`). The profile is applied by the core
// when the file is decoded; the UI never reads the file's rows itself.

/** One column as the core reads the header (tlmbench_decode::csv::InspectedColumn). */
export interface InspectedColumn {
  source: string;
  name: string;
  unit: string | null;
  kind: 'number' | 'text' | 'time';
}

export interface Inspection {
  header_key: string;
  delimiter: string;
  columns: InspectedColumn[];
  time: number | null;
  time_unit: string | null;
  preview: string[][];
  rows: number;
  rows_exact: boolean;
  rate_hz: number | null;
}

/** An import profile as the core takes it (tlmbench_decode::profile::ImportProfile). */
export interface Profile {
  time: { type: 'auto' } | { type: 'none' } | { type: 'column'; name: string; unit: string | null };
  rate: { type: 'as_recorded' } | { type: 'samples' } | { type: 'resample'; hz: number } | { type: 'assume'; hz: number };
  columns: { source: string; name?: string | null; unit?: string | null; kind?: 'number' | 'text' | 'skip' | null; category?: string | null }[];
}

export interface Found {
  exact: Profile | null;
  near: [number, { header: string[]; profile: Profile }] | null;
}

export interface Inspected {
  path: string;
  inspection: Inspection | null;
  error: string | null;
  found: Found;
}

export type RateMode = 'recorded' | 'fixed' | 'assume' | 'samples';
export type Kind = 'number' | 'text' | 'skip';
export type Where = 'tabs' | 'none';

export interface Col {
  source: string;
  name: string;
  unit: string;
  kind: Kind;
  category: string;
  /** What the core detected: the name, unit and kind the choices are measured against. */
  det: { name: string; unit: string; kind: Kind };
}

export interface Job {
  paths: string[];
  insp: Inspection;
  timeCol: string | null;
  unit: string;
  rateMode: RateMode;
  /** The rate chosen for Resample and Assume, in hertz. */
  rate: number;
  /** The unit the rate is shown in (a view choice; the profile keeps hertz). */
  rateUnit: string;
  cols: Col[];
  q: string;
  /** A saved profile for a header that nearly matches (offered, never applied unasked). */
  near: Found['near'];
  /** The name of what was applied from a similar file. */
  applied: string | null;
}

export const RATE_UNITS: [string, number][] = [
  ['kHz', 1e3],
  ['Hz', 1],
  ['per minute', 1 / 60],
  ['per hour', 1 / 3600],
];
export const rateUnitFor = (hz: number): string => (hz >= 1000 ? 'kHz' : hz >= 1 ? 'Hz' : hz >= 1 / 60 ? 'per minute' : 'per hour');
export const rateFactor = (unit: string): number => RATE_UNITS.find(([u]) => u === unit)?.[1] ?? 1;
export const fmtRate = (hz: number): string => {
  const u = rateUnitFor(hz);
  return `${+(hz / rateFactor(u)).toPrecision(4)} ${u === 'per minute' || u === 'per hour' ? u.replace('per ', '/') : u}`;
};

export const TIME_UNITS: [string, string][] = [
  ['s', 'seconds'],
  ['ms', 'milliseconds'],
  ['us', 'microseconds'],
  ['ns', 'nanoseconds'],
  ['iso', 'date and time text'],
];

export const imp = $state({
  open: false,
  jobs: [] as Job[],
  sel: 0,
  where: 'tabs' as Where,
  /** Files left out: what they are not. */
  bad: [] as { path: string; why: string }[],
  busy: false,
  error: '',
});

const toKind = (k: InspectedColumn['kind']): Kind => (k === 'number' ? 'number' : 'text');

/** A job for the files that share one header: its choices start from what the core detected. */
export function makeJob(paths: string[], insp: Inspection, near: Found['near']): Job {
  const t = insp.time == null ? null : insp.columns[insp.time].source;
  const cols = insp.columns.map((c) => {
    const kind = toKind(c.kind);
    return { source: c.source, name: c.name, unit: c.unit ?? '', kind, category: '', det: { name: c.name, unit: c.unit ?? '', kind } };
  });
  return {
    paths,
    insp,
    timeCol: t,
    unit: insp.time_unit ?? 's',
    rateMode: t == null ? 'samples' : 'recorded',
    rate: insp.rate_hz ?? 10,
    rateUnit: rateUnitFor(insp.rate_hz ?? 10),
    cols,
    q: '',
    near,
    applied: null,
  };
}

/** The profile a job's choices make. */
export function toProfile(job: Job): Profile {
  const time: Profile['time'] = job.timeCol == null ? { type: 'none' } : { type: 'column', name: job.timeCol, unit: job.unit };
  const rate: Profile['rate'] =
    job.rateMode === 'fixed' ? { type: 'resample', hz: job.rate } : job.rateMode === 'assume' ? { type: 'assume', hz: job.rate } : job.rateMode === 'samples' ? { type: 'samples' } : { type: 'as_recorded' };
  const columns: Profile['columns'] = [];
  for (const c of job.cols) {
    if (c.source === job.timeCol) continue;
    const ch: Profile['columns'][number] = { source: c.source };
    if (c.name.trim() && c.name.trim() !== c.det.name) ch.name = c.name.trim();
    if (c.unit.trim() !== c.det.unit) ch.unit = c.unit.trim();
    if (c.kind !== c.det.kind) ch.kind = c.kind;
    if (c.category.trim()) ch.category = c.category.trim();
    if (Object.keys(ch).length > 1) columns.push(ch);
  }
  return { time, rate, columns };
}

/** Takes a saved profile's choices for the columns this header has (a similar file's: the rest stay as detected). */
export function applyProfile(job: Job, p: Profile, label: string): void {
  const has = new Set(job.cols.map((c) => c.source));
  if (p.time.type === 'none') job.timeCol = null;
  else if (p.time.type === 'column' && has.has(p.time.name)) {
    job.timeCol = p.time.name;
    job.unit = p.time.unit ?? job.unit;
  }
  job.rateMode = p.rate.type === 'as_recorded' ? 'recorded' : p.rate.type === 'resample' ? 'fixed' : p.rate.type;
  if ('hz' in p.rate) {
    job.rate = p.rate.hz;
    job.rateUnit = rateUnitFor(p.rate.hz);
  }
  if (job.timeCol == null && job.rateMode === 'recorded') job.rateMode = 'samples';
  for (const ch of p.columns) {
    const c = job.cols.find((x) => x.source === ch.source);
    if (!c) continue;
    if (ch.name) c.name = ch.name;
    if (ch.unit != null) c.unit = ch.unit;
    if (ch.kind) c.kind = ch.kind;
    if (ch.category) c.category = ch.category;
  }
  job.applied = label;
}

/** Why a job can't open yet, or ''. */
export function problem(job: Job): string {
  if (job.timeCol == null && job.rateMode === 'recorded') return 'needs a time column';
  if ((job.rateMode === 'fixed' || job.rateMode === 'assume') && !(job.rate > 0)) return 'needs a rate above zero';
  if (job.rateMode === 'fixed' && job.timeCol == null) return 'needs a time column';
  return '';
}
