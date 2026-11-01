// Syntax-checks the prototype's main script (the last <script> block of plotter.html) with `node --check`.
import { readFileSync, writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

const html = readFileSync(new URL('../prototype/plotter.html', import.meta.url), 'utf8');
const blocks = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)];
if (!blocks.length) { console.error('proto-check: no <script> block in plotter.html'); process.exit(1); }
const file = join(mkdtempSync(join(tmpdir(), 'tlm-')), 'check.js');
writeFileSync(file, blocks.at(-1)[1]);
const r = spawnSync(process.execPath, ['--check', file], { stdio: 'inherit' });
if (r.status === 0) console.log('proto-check: plotter.html script parses');
process.exit(r.status ?? 1);
