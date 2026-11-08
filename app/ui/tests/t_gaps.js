// Settings → Gaps in lines: Break (the default) leaves a gap in the line; Connect asks the core to join across it,
// and the joined stretch is drawn dashed. The setting is saved and also applies to the overview.
import { run } from './lib.js';

run('gaps', async ({ p, check, until, fake, settingsLS }) => {
  await fake(() => { window.__tlmFake.gapAt = 100000; });
  await p.click('#b-settings');
  await p.click('#setPanel [data-sv="all"]');
  const ask = async (connect) => {
    await p.evaluate(() => { window.__tlmFake.connectAsked.length = 0; });
    await p.selectOption('[data-set="gapLines"]', connect ? 'connect' : 'break');
    return until(() => window.__tlmFake.connectAsked.length >= 2);
  };
  const dashes = () => p.evaluate(() => window.__tlmApp.dashes()[0]?.[0] ?? null);

  check('by default the setting is Break', (await settingsLS()).gapLines === undefined || (await settingsLS()).gapLines === 'break');
  // the jump exists from the next view on: Connect, then back to Break, makes one
  await ask(true);
  await ask(false);
  check('a break draws no dashed stretch', await until(() => window.__tlmApp.dashes()[0]?.[0]?.[0] === 0), await dashes());
  check('and cuts the line in two solid runs', (await dashes())?.[1] === 2, await dashes());

  check('Connect re-asks the charts and the overview', await ask(true), await fake(() => window.__tlmFake.connectAsked));
  check('with connectGaps on', await fake(() => window.__tlmFake.connectAsked.every(Boolean)));
  check('the join is drawn as a dashed stretch', await until(() => window.__tlmApp.dashes()[0]?.[0]?.[0] === 1), await dashes());
  check('the solid parts are two runs either side of it', (await dashes())?.[1] === 2, await dashes());
  check('it is saved in settings', (await settingsLS()).gapLines === 'connect');

  await ask(false);
  check('Break takes it back', await until(() => window.__tlmApp.dashes()[0]?.[0]?.[0] === 0), await dashes());
});
