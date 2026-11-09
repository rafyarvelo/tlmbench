// Derived channels in the UI: the recipe library as the core keeps it, the Derive… dialog's state, and the starting
// points (templates) the dialog offers. The core evaluates; the dialog only checks what is typed against the open file.

/** A recipe, as the core keeps it (`tlmbench_expr::Recipe`). */
export interface Recipe {
  name: string;
  expr: string;
  unit?: string | null;
  category?: string | null;
  decimals?: number | null;
  /** "Any file with these channels" gets it without being asked. */
  automatic: boolean;
}

/** A derived channel of the open file, from `open`. */
export interface DerivedInfo {
  channel: number;
  name: string;
  expr: string;
  automatic: boolean;
}

/** A starting expression: `X` is the selected channel, `A` and `B` two selected ones. */
export interface Template {
  id: string;
  label: string;
  expr: string;
  /** The unit the result has, from the first channel's. */
  unit: (u: string) => string;
}

export const TEMPLATES: Template[] = [
  { id: 'delta', label: 'Delta (change per sample)', expr: 'delta(X)', unit: (u) => u },
  { id: 'rate', label: 'Rate of change (per second)', expr: 'rate(X)', unit: (u) => (u ? `${u}/s` : '/s') },
  { id: 'pct', label: 'Returns (% change)', expr: 'pct_change(X) * 100', unit: () => '%' },
  { id: 'log', label: 'Log returns', expr: 'ln(X / prev(X))', unit: () => '' },
  { id: 'sma', label: 'Moving average (20 samples)', expr: 'sma(X, 20)', unit: (u) => u },
  { id: 'ema', label: 'Exponential average (20 samples)', expr: 'ema(X, 20)', unit: (u) => u },
  { id: 'vol', label: 'Rolling volatility (20, annualised %)', expr: 'rolling_std(pct_change(X), 20) * sqrt(252) * 100', unit: () => '%' },
  { id: 'dd', label: 'Drawdown %', expr: '(X / running_max(X) - 1) * 100', unit: () => '%' },
  { id: 'scale', label: 'Scale and offset', expr: 'X * 1 + 0', unit: (u) => u },
  { id: 'abs', label: 'Absolute value', expr: 'abs(X)', unit: (u) => u },
  { id: 'cum', label: 'Cumulative sum', expr: 'cumsum(X)', unit: (u) => u },
  { id: 'int', label: 'Integral over time', expr: 'integral(X)', unit: (u) => (u ? `${u}·s` : '') },
  { id: 'mean', label: 'Mean of the whole file (one number)', expr: 'mean(X)', unit: (u) => u },
  { id: 'custom', label: 'Custom expression', expr: 'X', unit: (u) => u },
];

export const TEMPLATES2: Template[] = [
  { id: 'diff2', label: 'A − B', expr: 'A - B', unit: (a) => a },
  { id: 'ratio', label: 'A ÷ B', expr: 'A / B', unit: () => '' },
  { id: 'prod', label: 'A × B', expr: 'A * B', unit: () => '' },
  { id: 'sum2', label: 'A + B', expr: 'A + B', unit: (a) => a },
];

const FUNCTIONS = new Set(['abs', 'sqrt', 'ln', 'log', 'log10', 'exp', 'pow', 'min', 'max', 'clamp', 'round', 'floor', 'ceil', 'sign', 'sin', 'cos', 'tan', 'asin', 'acos', 'atan', 'atan2', 'deg', 'rad', 'if', 'delta', 'diff', 'prev', 'pct_change', 'rate', 'cumsum', 'integral', 'running_max', 'running_min', 'sma', 'rolling_mean', 'rolling_std', 'rolling_min', 'rolling_max', 'rolling_sum', 'ema', 'movavg', 'median', 'lowpass', 'highpass', 'bandpass', 'interp', 'mean', 'sum', 'std', 'first', 'last', 'count', 'pct', 'duration', 'occurrences', 'since', 'past_warn', 'past_alarm', 'pi', 'e']);

/** A channel's name as an expression writes it: bare when it can be, else in quotes. */
export const quoteName = (n: string) => (/^[A-Za-z_][A-Za-z0-9_.]*$/.test(n) && !FUNCTIONS.has(n.toLowerCase()) ? n : `"${n}"`);

/** The functions, for autocomplete and the dialog's help. */
export const FUNCTION_NAMES = [...FUNCTIONS].filter((f) => f !== 'pi' && f !== 'e').sort();

export const FN_HELP =
  'Math: + − × ÷ ^ %, abs sqrt ln log10 exp pow min max clamp round floor ceil sign sin cos tan atan2 deg rad, if(cond, a, b). ' +
  'Sample to sample: delta prev(x, n) pct_change rate cumsum integral running_max running_min. Rolling (20 samples or 30d): sma ema rolling_std rolling_min rolling_max rolling_sum. ' +
  'Filters: lowpass(x, Hz) highpass(x, Hz) bandpass(x, Hz, Hz) movavg(x, 20) median(x, 5); interp(x) resamples linearly. ' +
  'One number: mean std min max first last sum, pct(cond) duration(cond) occurrences(cond). since(cond): seconds since it last held. ' +
  'Names with spaces go in quotes; "Close|Adj Close" takes whichever exists.';

/** The recipe library, as the core has it. */
export const recipes = $state({ list: [] as Recipe[] });

/** What the Derive… dialog is doing. `editing` is the recipe's name when it edits one. */
export const derive = $state({
  open: false,
  editing: null as string | null,
  busy: false,
  /** The names of the channels it starts from (none, one, or two for A − B and the like), and their units. */
  base: [] as { name: string; unit: string }[],
  tpl: 'delta',
  name: '',
  expr: '',
  unit: '',
  category: 'Derived',
  decimals: 3,
  /** Where it applies: this file, any file with these channels, or (editing) where it applies now. */
  scope: 'file' as 'file' | 'auto' | 'keep',
  /** The check of the expression against the open file: what to say, whether it can be saved, and a preview's numbers. */
  status: { ok: false, text: '', pending: false },
  error: '',
});
