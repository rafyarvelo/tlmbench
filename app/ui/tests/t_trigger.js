// Live triggers and live checks: arm a trigger on a channel or an expression (level or edge, pre and post time, single
// or repeat); when it fires the view holds the window around the crossing and an event in the Trigger category marks
// it; Follow goes back to live. Rules and checks run again every couple of seconds on a live file.
import { run } from './lib.js';

const TCP = 'tcp://127.0.0.1:9000';
const SET = {
  schema: 1,
  name: 'Bench',
  runs_on_any_file: true,
  info_groups: [],
  check: [{ name: 'RPM under 9000', kind: 'check', group: 'Propulsion', expect: 'rpm < 9000', severity: 'fail', grace: 0, settle: 0, enabled: true }],
};

run(
  'trigger',
  async ({ p, check, text, fake, until, settingsLS, wait }) => {
    const span = (v) => Number(BigInt(v.t1) - BigInt(v.t0)) / 1e9;
    const view = () => p.evaluate(() => window.__tlmApp.view());
    const set = async (sel, v) => {
      await p.fill(sel, String(v));
      await p.dispatchEvent(sel, 'change');
    };

    await p.click('[data-rail="live"]');
    await p.click('#liveDlg [data-kind="tcp"]');
    await p.fill('#lvAddr', '127.0.0.1:9000');
    await p.fill('#lvDesc', '/data/frames.toml');
    await p.click('#lvConnect');
    await until(() => !!document.getElementById('liveStrip'));
    check('Trigger… in the live strip', (await text('#lvTrig')) === 'Trigger…');

    // a trigger that can't be armed says why
    await p.click('#lvTrig');
    check('the popover lists the live channels', (await p.$$eval('#lvChans option', (o) => o.map((x) => x.value))).join() === 'rpm,temp,volts');
    await p.fill('.trigpop [data-tg="ch"]', 'cumsum(rpm)');
    await p.click('.trigpop [data-tgarm]');
    check('a channel that needs the whole recording is refused, with the reason', await until(() => /can't trigger live/.test(document.getElementById('lvTrigErr')?.textContent ?? '')), await text('.trigpop'));
    await p.fill('.trigpop [data-tg="ch"]', 'nothing');
    await p.click('.trigpop [data-tgarm]');
    check('so is one that isn\'t there', await until(() => /no channel called nothing/.test(document.getElementById('lvTrigErr')?.textContent ?? '')), await text('.trigpop'));
    check('...and it stays unarmed', (await text('#lvTrig')) === 'Trigger…');

    // a single trigger: fires once, the view holds the window around it, an event marks it
    await p.fill('.trigpop [data-tg="ch"]', 'rpm');
    await p.selectOption('.trigpop [data-tg="kind"]', 'rise');
    await set('.trigpop [data-tg="level"]', 1500);
    await set('.trigpop [data-tg="pre"]', 1);
    await set('.trigpop [data-tg="post"]', 1);
    await p.click('.trigpop [data-tgarm]');
    check('arming closes the popover and the button says Armed', !(await p.$('.trigpop')) && /^Armed/.test(await text('#lvTrig')), await text('#lvTrig'));
    check('the core was asked for exactly that', JSON.stringify(await fake(() => window.__tlmFake.triggers.at(-1))) === JSON.stringify({ channel: 'rpm', kind: 'rise', level: 1500, pre: 1, post: 1, repeat: false }), await fake(() => window.__tlmFake.triggers.at(-1)));
    check('what was chosen is kept for next time', JSON.stringify((await settingsLS()).liveTrigger) === JSON.stringify({ channel: 'rpm', kind: 'rise', level: 1500, pre: 1, post: 1, repeat: false }), (await settingsLS()).liveTrigger);
    check('it fires once and freezes the view', await until(() => /Triggered · 1×/.test(document.getElementById('lvTrig')?.textContent ?? ''), null, 8000) && (await until(() => document.getElementById('lvFollow')?.textContent === 'Jump to live', null, 4000)), await text('#lvTrig'));
    const v = await view();
    check('the view is the window around it: 1 s before, 1 s after', Math.abs(span(v) - 2) < 0.01, span(v));
    await wait(1500);
    check('it stays frozen, and a single trigger fires only once', JSON.stringify(await view()) === JSON.stringify(v) && /Triggered · 1×/.test(await text('#lvTrig')));
    await p.click('[data-rail="events"]');
    check('an event in the Trigger category marks it', await until(() => /Trigger: rpm rises through 1500/.test(document.getElementById('eventsPanel')?.textContent ?? '') && /Trigger/.test(document.getElementById('eventsPanel')?.textContent ?? '')), await text('#eventsPanel'));
    await p.click('#lvFollow');
    check('Follow returns to live', (await text('#lvFollow')) === 'Following');

    // the same trigger as an event rule, to mark crossings in a recorded file
    await p.click('#lvTrig');
    await p.click('.trigpop [data-tgrule]');
    check('Make a rule… opens the rule editor with the crossing as its condition', await until(() => /prev\(rpm\) < 1500 && rpm >= 1500/.test(document.querySelector('.ckdlg, #checkEditor, [role=dialog]')?.innerText ?? '') || [...document.querySelectorAll('input,textarea')].some((i) => /prev\(rpm\) < 1500/.test(i.value))), await text('[role=dialog]'));
    await p.keyboard.press('Escape');

    // repeat: fires again and shows the latest
    await p.click('#lvTrig');
    check('the popover opens with what is armed', (await p.inputValue('.trigpop [data-tg="ch"]')) === 'rpm' && (await p.inputValue('.trigpop [data-tg="level"]')) === '1500');
    await p.selectOption('.trigpop [data-tg="repeat"]', '1');
    await p.selectOption('.trigpop [data-tg="kind"]', 'fall');
    await p.click('.trigpop [data-tgarm]');
    check('arming again starts counting afresh', /^Armed/.test(await text('#lvTrig')), await text('#lvTrig'));
    check('a repeating trigger fires again and again, and stays armed', await until(() => /Triggered · [2-9]×/.test(document.getElementById('lvTrig')?.textContent ?? '') || /Armed · [2-9]×/.test(document.getElementById('lvTrig')?.textContent ?? ''), null, 12000), await text('#lvTrig'));
    await p.click('#lvTrig');
    check('Turn off stops it', await p.isVisible('.trigpop [data-tgoff]'));
    await p.click('.trigpop [data-tgoff]');
    check('the button no longer says Armed', await until(() => !/^Armed/.test(document.getElementById('lvTrig')?.textContent ?? '')), await text('#lvTrig'));

    // rules and checks run again on a live file every couple of seconds
    await fake((s) => (window.__tlmFake.checkSets = [s]), SET);
    await p.click('[data-rail="checks"]');
    await p.evaluate(() => window.__tlmFake.checkRuns.splice(0));
    await p.click('#lvFollow').catch(() => {});
    await wait(5200);
    const runs = (await fake(() => window.__tlmFake.checkRuns)).filter((r) => r.path === TCP && r.sets.includes('Bench'));
    check('the checks run on the live file again and again', runs.length >= 2 && runs.length <= 5, runs.length);
  },
  {
    files: { '/data/frames.toml': {} },
    launch: null,
    setup: (fake) => {
      fake.sources['tcp://127.0.0.1:9000'] = { channels: ['rpm', 'temp', 'volts'], rate: 1000, already: 40 };
      fake.triggerAfter = 1500;
    },
  },
);
