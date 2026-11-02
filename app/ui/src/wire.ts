// Decodes the core's binary viewport reply (see crates/core/src/wire.rs) into Float32Array views of the same
// buffer: no parsing, no copies.

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

export function decode(buf: ArrayBuffer): Reply {
  const v = new DataView(buf);
  const magic = String.fromCharCode(v.getUint8(0), v.getUint8(1), v.getUint8(2), v.getUint8(3));
  if (magic !== 'TLMV' || v.getUint16(4, true) !== 2) throw new Error('not a viewport reply');
  const count = v.getUint32(12, true);
  const traces: Trace[] = [];
  let o = HEADER;
  for (let i = 0; i < count; i++) {
    const n = v.getUint32(o + 4, true);
    const kind = v.getUint32(o + 8, true);
    const data = o + TRACE_HEADER;
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
    o = data + 8 * n; // x and y are 4n bytes each, so traces stay 8-byte aligned
  }
  return {
    generation: v.getUint32(8, true),
    originNs: v.getBigInt64(16, true),
    xScaleNs: v.getFloat64(24, true),
    builtUntil: v.getUint16(6, true) & 1 ? v.getBigInt64(32, true) : null,
    traces,
    bytes: buf.byteLength,
  };
}
