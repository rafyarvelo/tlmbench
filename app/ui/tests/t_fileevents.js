// Events from the file (D-ui-12): a CSV column marked Events in the import dialog (kept in the header's profile) or a
// descriptor's `events` makes one event at each change of its value; the core makes them (file_events), the Events panel
// lists them as "From the file", and the panel can group by file.
import { run } from './lib.js';

run(
  'fileevents',
  async ({ p, check, fake, until, text }) => {
    await fake(() => {
      const f = window.__tlmFake;
      f.csv('/data/log.csv', ['t', 'a', 'mode'], { unit: 's', rate: 10 });
      f.files['/data/log.csv'] = {};
      f.fileEvents['/data/log.csv'] = [
        { id: 'file-1-20', name: 'Cruise', start: '2000000000' },
        { id: 'file-1-40', name: 'Landing', start: '4000000000' },
      ];
      f.drop(['/data/log.csv']);
    });
    check('the dialog asks', await until(() => !!document.querySelector('#importDlg')));
    check('every column has an Events box', (await p.$$('#impCols [data-evcol]')).length === 2);
    await p.check('#impCols [data-h="mode"] [data-evcol]');
    await p.click('#impGo');
    check('the profile keeps the marked column', await until(() => (window.__tlmFake.saved.at(-1)?.profile.event_columns ?? []).join() === 'mode'), await fake(() => window.__tlmFake.saved.at(-1)));
    check("the core is asked for that channel's events", await until(() => window.__tlmFake.fileEventsAsked.length >= 1));
    await p.click('[data-rail="events"]');
    check('the events are listed', await until(() => /Cruise/.test(document.querySelector('#eventsPanel')?.textContent ?? '') && /Landing/.test(document.querySelector('#eventsPanel')?.textContent ?? '')), await text('#eventsPanel'));
    await p.selectOption('#evGroup', 'source');
    check('grouped by source they are From the file', await until(() => /From the file/.test(document.querySelector('#eventsPanel')?.textContent ?? '')));
    await p.selectOption('#evGroup', 'file');
    check('and the panel can group by file', await until(() => /log\.csv/.test(document.querySelector('#eventsPanel [data-evg]')?.textContent ?? '')), await text('#eventsPanel'));
  },
  { files: { '/data/log.csv': {} }, launch: null },
);
