// The editor's Lists mode: the simple checks and rules read as "every sample of · channel · > · value" and never need
// an expression typed. `parsePick` reads an expression as a pick (null when it's anything more), `buildPick` writes one.

import { quoteName } from './derive.svelte';

export type Agg = 'each' | 'max' | 'min' | 'mean' | 'abs';
export type Op = '<' | '<=' | '>' | '>=' | '==' | '!=' | 'is' | 'isnt' | 'outside' | 'warn' | 'alarm';

export interface Pick {
  agg: Agg;
  ch: string;
  op: Op;
  /** The value (or the label for is / is not, or the low edge of a band). */
  v: string;
  /** The high edge of a band. */
  v2: string;
}

const NAME = '"[^"]+"|[A-Za-z_][A-Za-z0-9_.]*';
const NUM = '-?\\d+(?:\\.\\d+)?(?:[eE][-+]?\\d+)?';
const unq = (n: string) => n.replace(/^"|"$/g, '');
const same = (a: string, b: string) => unq(a) === unq(b);

export function parsePick(text: string): Pick | null {
  const t = text.trim();
  let m: RegExpExecArray | null;
  if ((m = new RegExp(`^past_(warn|alarm)\\(\\s*(${NAME})\\s*\\)$`).exec(t))) return { agg: 'each', ch: unq(m[2]), op: m[1] as Op, v: '', v2: '' };
  if ((m = new RegExp(`^(${NAME})\\s*<\\s*(${NUM})\\s*\\|\\|\\s*(${NAME})\\s*>\\s*(${NUM})$`).exec(t)) && same(m[1], m[3])) return { agg: 'each', ch: unq(m[1]), op: 'outside', v: m[2], v2: m[4] };
  if ((m = new RegExp(`^(${NAME})\\s*(==|!=)\\s*'([^']*)'$`).exec(t))) return { agg: 'each', ch: unq(m[1]), op: m[2] === '==' ? 'is' : 'isnt', v: m[3], v2: '' };
  if ((m = new RegExp(`^(abs|max|min|mean)\\(\\s*(${NAME})\\s*\\)\\s*(<=|>=|<|>|==|!=)\\s*(${NUM})$`).exec(t))) return { agg: m[1] as Agg, ch: unq(m[2]), op: m[3] as Op, v: m[4], v2: '' };
  if ((m = new RegExp(`^(${NAME})\\s*(<=|>=|<|>|==|!=)\\s*(${NUM})$`).exec(t))) return { agg: 'each', ch: unq(m[1]), op: m[2] as Op, v: m[3], v2: '' };
  return null;
}

export function buildPick(p: Pick): string {
  const c = quoteName(p.ch);
  switch (p.op) {
    case 'warn':
    case 'alarm':
      return `past_${p.op}(${c})`;
    case 'outside': {
      const [a, b] = [+p.v, +p.v2].sort((x, y) => x - y);
      return `${c} < ${a} || ${c} > ${b}`;
    }
    case 'is':
    case 'isnt':
      return `${c} ${p.op === 'is' ? '==' : '!='} '${p.v}'`;
    default:
      return `${p.agg === 'each' ? c : `${p.agg}(${c})`} ${p.op} ${p.v === '' ? 0 : p.v}`;
  }
}
