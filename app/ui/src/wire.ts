// Decodes the core's binary viewport replies (see crates/core/src/wire.rs) into Float32Array views of the same
// buffer: no parsing, no copies. A `view` call answers with a batch: one reply per pane and the overview.

export interface Trace {
  channel: number;
  kind: 'raw' | 'm4';
  /** Drawn from whole summaries; an exact reply replaces it. */
  coarse: boolean;
  fromPyramid: boolean;
  /** Time the core spent on the request, in microseconds. */
  reduceUs: number;
  yMin: number;
  yMax: number;
  x: Float32Array;
  y: Float32Array;
}

export interface Reply {
  generation: number;
  /** A sample's time is originNs + x * xScaleNs. */
  originNs: bigint;
  xScaleNs: number;
  /** Samples after this time aren't summarized yet (null: built, or no build). */
  builtUntil: bigint | null;
  traces: Trace[];
  bytes: number;
}

const HEADER = 40;
const TRACE_HEADER = 24;

export interface Batch {
  generation: number;
  /** Replies by entry id (the pane number, or OVERVIEW). */
  entries: Map<number, Reply>;
  bytes: number;
}

const BATCH_HEADER = 16;
const BATCH_ENTRY = 16;

export function decodeBatch(buf: ArrayBuffer): Batch {
  const v = new DataView(buf);
  const magic = String.fromCharCode(v.getUint8(0), v.getUint8(1), v.getUint8(2), v.getUint8(3));
  if (magic !== 'TLMB' || v.getUint16(4, true) !== 1) throw new Error('not a view batch');
  const count = v.getUint32(12, true);
  const entries = new Map<number, Reply>();
  for (let k = 0; k < count; k++) {
    const e = BATCH_HEADER + BATCH_ENTRY * k;
    entries.set(v.getUint32(e, true), decode(buf, v.getUint32(e + 8, true), v.getUint32(e + 12, true)));
  }
  return { generation: v.getUint32(8, true), entries, bytes: buf.byteLength };
}

/** One reply: `len` bytes at `at` (8-byte aligned) of `buf`. */
export function decode(buf: ArrayBuffer, at = 0, len = buf.byteLength): Reply {
  const v = new DataView(buf, at, len);
  const magic = String.fromCharCode(v.getUint8(0), v.getUint8(1), v.getUint8(2), v.getUint8(3));
  if (magic !== 'TLMV' || v.getUint16(4, true) !== 2) throw new Error('not a viewport reply');
  const count = v.getUint32(12, true);
  const traces: Trace[] = [];
  let o = HEADER;
  for (let i = 0; i < count; i++) {
    const n = v.getUint32(o + 4, true);
    const kind = v.getUint32(o + 8, true);
    const data = at + o + TRACE_HEADER;
    traces.push({
      channel: v.getUint32(o, true),
      kind: kind & 1 ? 'm4' : 'raw',
      coarse: (kind & 2) !== 0,
      fromPyramid: (kind & 4) !== 0,
      reduceUs: v.getUint32(o + 12, true),
      yMin: v.getFloat32(o + 16, true),
      yMax: v.getFloat32(o + 20, true),
      x: new Float32Array(buf, data, n),
      y: new Float32Array(buf, data + 4 * n, n),
    });
    o += TRACE_HEADER + 8 * n; // x and y are 4n bytes each, so traces stay 8-byte aligned
  }
  return {
    generation: v.getUint32(8, true),
    originNs: v.getBigInt64(16, true),
    xScaleNs: v.getFloat64(24, true),
    builtUntil: v.getUint16(6, true) & 1 ? v.getBigInt64(32, true) : null,
    traces,
    bytes: len,
  };
}

/** A `rows` reply: a window of the raw samples in a span (see the app's `rows` command). */
export interface Rows {
  /** The first row's sample index in the recording. */
  first: number;
  /** Rows in the whole span. */
  total: number;
  times: BigInt64Array;
  /** One column per channel asked for, in order (NaN where missing). */
  values: Float64Array[];
}

const ROWS_HEADER = 32;

export function decodeRows(buf: ArrayBuffer): Rows {
  const v = new DataView(buf);
  if (String.fromCharCode(...new Uint8Array(buf, 0, 4)) !== 'TLMR') throw new Error('not a rows reply');
  const channels = v.getUint32(4, true);
  const n = v.getUint32(24, true);
  const times = new BigInt64Array(buf, ROWS_HEADER, n);
  const values = Array.from({ length: channels }, (_, k) => new Float64Array(buf, ROWS_HEADER + 8 * n * (k + 1), n));
  return { first: Number(v.getBigUint64(8, true)), total: Number(v.getBigUint64(16, true)), times, values };
}
