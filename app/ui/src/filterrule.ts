// A table's column filters as the expression of a rule ("Save as rule", S5.3-14). The filter language is the core's
// (`crates/core/src/table.rs`): `> 1.5`, `1..2`, `blank`, joined with and / or. A rule says the same in the check
// language: `accel_x > 1 && accel_x < 2`. Blank and not-blank tests have no rule form and are left out.

const NUM = /^-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?$/;

/** The channel name as an expression spells it: bare when it can be, else in quotes. */
export const quoted = (name: string): string => (/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) ? name : `"${name}"`);

function atom(name: string, s: string): string | null {
  if (['blank', 'empty', 'missing', '!blank', 'not blank', '!empty', 'not empty'].includes(s)) return null;
  const r = s.split('..');
  if (r.length === 2 && NUM.test(r[0].trim()) && NUM.test(r[1].trim())) {
    const [a, b] = [Number(r[0]), Number(r[1])];
    return `${name} >= ${Math.min(a, b)} && ${name} <= ${Math.max(a, b)}`;
  }
  const m = /^(<=|>=|!=|==|>|<|=)?\s*(.+)$/.exec(s);
  if (!m || !NUM.test(m[2].trim())) return null;
  const op = m[1] === '=' || !m[1] ? '==' : m[1];
  return `${name} ${op} ${Number(m[2])}`;
}

const split = (s: string, seps: RegExp): string[] => s.split(seps).map((x) => x.trim()).filter(Boolean);

/** One column's filter text as an expression, or null when it has nothing a rule can test. */
export function columnExpr(channel: string, text: string): string | null {
  const name = quoted(channel);
  const groups = split(text.toLowerCase(), / or |\|\|/)
    .map((g) => split(g, / and |&&/).map((a) => atom(name, a)).filter((a): a is string => !!a))
    .filter((g) => g.length)
    .map((g) => g.join(' && '));
  return groups.length ? (groups.length > 1 ? `(${groups.join(' || ')})` : groups[0]) : null;
}

/** Every column's filter, all of which must pass. */
export function filterExpr(filters: { channel: string; text: string }[]): string {
  return filters.map((f) => columnExpr(f.channel, f.text)).filter(Boolean).join(' && ');
}
