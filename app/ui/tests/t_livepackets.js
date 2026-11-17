// A live packet stream (ledger D-ingest-11, owner Nov 16): one connection, a source per packet type, each in a live tab
// of its own named for its type. The fake core stands in for tlmbench-live (its own test: packets::tests).
import { run } from './lib.js';

const TCP = 'tcp://127.0.0.1:9100';

run('livepackets', async ({ p, check, text, fake, until }) => {
  await fake((e) => (window.__tlmFake.sources[e] = { channels: ['count', 'half'], rate: 100, already: 5, types: ['imu', 'status'] }), TCP);
  await p.click('[data-rail="live"]');
  await p.click('#liveDlg [data-kind="tcp"]');
  await p.fill('#lvHost', '127.0.0.1');
  await p.fill('#lvPortNo', '9100');
  await p.fill('#lvDesc', '/data/rover.toml');
  await p.click('#lvConnect');
  const tabs = () => p.$$eval('#wsTabs [role="tab"]', (t) => t.map((x) => x.textContent.replace(/\s+/g, ' ').trim()));
  await until(() => document.querySelectorAll('[data-live-tab]').length === 2);
  const names = await tabs();
  check('a tab per packet type, each named for its type', names.some((t) => t.includes('127.0.0.1:9100 · imu')) && names.some((t) => t.includes('127.0.0.1:9100 · status')), names);
  check('from one connection', (await fake(() => window.__tlmFake.liveConnects.length)) === 1, await fake(() => window.__tlmFake.liveConnects));
  check('both are live tabs, the first in front', (await p.$$('[data-live-tab]')).length === 2 && /imu/.test(await text('#wsTabs [aria-selected=true]')), await tabs());
  await p.click('#wsTabs [role="tab"]:has-text("status")');
  check('the other type\'s tab follows its own source', await until(() => window.__tlmApp.drawn().every((d) => d && d.length)));
});
