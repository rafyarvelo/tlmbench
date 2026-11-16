// A CSV text column opens as a labelled channel (ledger D-ingest-12, owner Nov 16): its values read as its texts in
// readouts and tables, its axis has a tick per text when it is alone on it, and a value map the person typed still wins.
// The core's side (codes in first-seen order, more than 256 texts left out with a note) is tlmbench-decode's
// `text_columns_open_as_labelled_channels_up_to_256_texts`.
import { run } from './lib.js';

const F = '/data/modes.tlm';

run(
  'textlabels',
  async ({ p, check, until }) => {
    const app = (f, ...a) => p.evaluate(([f, a]) => window.__tlmApp[f](...a), [f, a]);
    check('a code reads as its text', (await app('fmt', 0, 1)) === 'Mission' && (await app('fmt', 0, 2)) === 'Return', [await app('fmt', 0, 1), await app('fmt', 0, 2)]);
    check('a number channel is unchanged', (await app('fmt', 1, 2)) !== 'Return');
    check('a pane with the text channel alone has a tick per text', await until(() => JSON.stringify(window.__tlmApp.axisLabels(0)) === '["Idle","Mission","Return"]'), await app('axisLabels', 0));
    check('a pane of number channels has number ticks', (await app('axisLabels', 1)) === null, await app('axisLabels', 1));
    // a value map the person typed is over the file's own texts
    await app('setValueMap', 'mode', { 1: 'Auto mission' });
    check('a typed value map wins', (await app('fmt', 0, 1)) === 'Auto mission' && (await app('fmt', 0, 0)) === 'Idle', [await app('fmt', 0, 1), await app('fmt', 0, 0)]);
  },
  { files: { [F]: { channels: ['mode', 'rpm'], labels: { mode: ['Idle', 'Mission', 'Return'] } } }, settings: { fileOpenNew: 'starter' } },
);
