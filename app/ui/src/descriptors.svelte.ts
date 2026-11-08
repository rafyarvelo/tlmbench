// The descriptor form's state: the descriptor being edited (the JSON of the core's `Descriptor`, so what is sent is what
// the core parses), the descriptors it knows, and what the core says about the descriptor against the file's first
// bytes (`descriptor_preview`: decoded rows, problems worded as the decoder words them, the TOML). The form computes
// nothing over samples; it shows what comes back.

export type DType = 'i8' | 'u8' | 'i16' | 'u16' | 'i32' | 'u32' | 'i64' | 'u64' | 'f32' | 'f64';
export const SIZES: Record<DType, number> = { i8: 1, u8: 1, i16: 2, u16: 2, i32: 4, u32: 4, i64: 8, u64: 8, f32: 4, f64: 8 };
export const TYPES = Object.keys(SIZES) as DType[];
export const INT_TYPES: DType[] = ['u8', 'u16', 'u32', 'u64'];
export const TIME_UNITS: [string, string][] = [['ns', 'ns'], ['us', 'µs'], ['ms', 'ms'], ['s', 's']];
export const CHECKS: [string, string][] = [['none', 'None'], ['sum8', 'Sum (8-bit)'], ['crc16-ccitt', 'CRC-16/CCITT'], ['crc32', 'CRC-32']];
export const COMPRESSIONS: [string, string][] = [['none', 'None'], ['zstd', 'zstd'], ['lz4', 'LZ4'], ['gzip', 'gzip']];
const CHECK_SIZE: Record<string, number> = { none: 0, sum8: 1, 'crc16-ccitt': 2, crc32: 4 };

export interface Bits {
  shift: number;
  width: number;
}
export interface DField {
  name: string;
  type: DType;
  offset: number;
  unit?: string;
  scale?: number;
  value_offset?: number;
  bits?: Bits;
}
export type Time =
  | { source: 'rate'; rate_hz: number; t0_ns?: number }
  | { source: 'field'; field: string; unit: string; t0_ns?: number }
  | { source: 'counter'; field: string; clock_hz: number; wraps_at?: number; t0_ns?: number }
  | { source: 'index' };
export interface Packets {
  sync?: string;
  header_size: number;
  length?: { offset: number; type: DType; counts: 'payload' | 'packet'; adjust?: number };
  id?: { offset: number; type: DType };
  checksum: string;
  compression: string;
}
export interface Descriptor {
  schema: 1;
  name: string;
  files: string[];
  endian: 'little' | 'big';
  header_size: number;
  record_size: number;
  sequence?: string;
  time: Time;
  fields: DField[];
  packets?: Packets;
  /** Kept as they are: the form doesn't edit packet types (each is its own recording). */
  packet_types?: unknown[];
}

/** A descriptor file the core knows (tlmbench_decode::descriptor::Known), and whether it names the file. */
export interface Known {
  id: string;
  path: string;
  shared: boolean;
  descriptor: Partial<Descriptor>;
  claims: boolean;
}

/** One decoded record (tlmbench_decode::preview::Row). */
export interface PreviewRow {
  offset: number;
  time: number | null;
  values: (number | null)[];
  ok: boolean | null;
}
export interface Preview {
  toml: string;
  problem: string | null;
  error: string | null;
  rows: PreviewRow[];
  stride: number;
  start: number;
  skipped: number;
  bad_checks: number;
  checked: boolean;
  hex_at: number;
  hex: number[];
}
export interface Frames {
  sync: string;
  stride: number;
  start: number;
}

export const dsc = $state({
  open: false,
  path: '',
  d: blankFor('') as Descriptor,
  /** The file (path) of the known descriptor loaded into the form ('' for a new one). */
  pick: '',
  /** Whether it is saved in the app cache (a shared one is copied there when saved). */
  pickShared: false,
  /** The record size follows the fields until a size is typed. */
  autoSize: true,
  /** The last Find frames: what it found, or false for nothing. */
  found: null as Frames | null | false,
  tomlOpen: false,
  known: [] as Known[],
  pv: null as Preview | null,
  /** The descriptor the preview was made from (the form may have moved on since): its columns and bytes' colours. */
  pvD: null as Descriptor | null,
  busy: false,
  error: '',
  /** Share… asks once before it writes into the folder. */
  confirmShare: false,
});

/** The descriptor being edited, as plain data (what the commands take). */
export function editing(): Descriptor {
  return $state.snapshot(dsc.d) as Descriptor;
}

/** esc_bench_0930.bin → esc_bench_*.bin: the files from the same logger. */
export function namePattern(name: string): string {
  return name.replace(/\d+/g, '*').replace(/\*+/g, '*');
}
export function nameOf(p: string): string {
  return p.split(/[\\/]/).pop() ?? p;
}

/** A new descriptor for a file: named after it, claiming its siblings, one u16 field. */
export function blankFor(path: string): Descriptor {
  const name = nameOf(path);
  return {
    schema: 1,
    name: name.replace(/\.[^.]+$/, '').replace(/[_-]?\d+$/, '') || 'New format',
    files: path ? [namePattern(name)] : [],
    endian: 'little',
    header_size: 0,
    record_size: 2,
    time: { source: 'rate', rate_hz: 100 },
    fields: [{ name: 'value', type: 'u16', offset: 0 }],
  };
}

/** A descriptor as the core serializes it (defaults left out), with them filled in for editing. */
export function normalizeDescriptor(d: Partial<Descriptor>): Descriptor {
  const out = structuredClone($state.snapshot(d)) as Partial<Descriptor>;
  return {
    schema: 1,
    name: out.name ?? '',
    files: out.files ?? [],
    endian: out.endian ?? 'little',
    header_size: out.header_size ?? 0,
    record_size: out.record_size ?? 0,
    time: out.time ?? { source: 'index' },
    fields: out.fields ?? [],
    ...(out.sequence ? { sequence: out.sequence } : {}),
    ...(out.packets ? { packets: { ...out.packets, checksum: out.packets.checksum ?? 'none', compression: out.packets.compression ?? 'none' } } : {}),
    ...(out.packet_types?.length ? { packet_types: out.packet_types } : {}),
  };
}

export const hexLen = (sync: string | undefined): number => {
  const h = String(sync ?? '').replace(/^0x/i, '').replace(/[\s_]/g, '');
  return /^([0-9a-f]{2})+$/i.test(h) ? h.length / 2 : 0;
};
export const isPackets = (d: Descriptor): boolean => !!d.packets;
export const checkSize = (d: Descriptor): number => CHECK_SIZE[d.packets?.checksum ?? 'none'] ?? 0;

/** The bytes a record takes: the largest field end, less a packet's header (the fields count from the packet start). */
export const fieldsEnd = (d: Descriptor): number => Math.max(1, ...d.fields.map((f) => f.offset + SIZES[f.type] - (d.packets?.header_size ?? 0)));

/** The bits a field takes as the form types them: `4-6` (counting from the least significant bit), or '' for all of them. */
export const bitsText = (b: Bits | undefined): string => (b ? (b.width === 1 ? String(b.shift) : `${b.shift}-${b.shift + b.width - 1}`) : '');
/** Parses `bitsText` back; undefined for a blank, null for text that isn't a bit range. */
export function parseBits(text: string): Bits | undefined | null {
  const t = text.trim();
  if (!t) return undefined;
  const m = /^(\d+)\s*(?:[-–:]\s*(\d+))?$/.exec(t);
  if (!m) return null;
  const lo = +m[1];
  const hi = m[2] == null ? lo : +m[2];
  return hi < lo ? null : { shift: lo, width: hi - lo + 1 };
}

/** Each field's colour in the hex and the field list. */
export const swatch = (i: number): string => `var(--trace-${(i % 8) + 1})`;
