// Code widgets: inputs by role and parameters (S5.4-10). A widget's manifest lists roles (`roll`, `pitch`) and parameters
// (`gain`); its menu has "Inputs and settings…", where each role is bound to a channel and each parameter valued, kept
// with the pane's layout.
import { run } from './lib.js';

const TILT = {
  id: 'cw_tilt',
  name: 'Tilt (code)',
  code: '{ build(el, info) { el.textContent = info.roles.join(); }, update(el, x) { el.textContent = x.params.gain; } }',
  inputs: [{ role: 'roll' }, { role: 'pitch', label: 'Pitch channel' }],
  params: [{ name: 'gain', label: 'Gain', type: 'number', default: '2' }, { name: 'mode', type: 'choice', choices: ['fast', 'slow'], default: 'fast' }],
};

run(
  'widgetinputs',
  async ({ p, check, until, settingsLS }) => {
    await p.focus('#explorer .row.chan[data-ch="0"]');
    await p.keyboard.press('ControlOrMeta+a');
    await p.click('#selBar [data-sel="chart"]');
    const idx = await p.evaluate(() => window.__tlmApp.sel()); // the new pane, below the selected one
    await p.click(`#stack [data-pane="${idx}"] [data-vmenu]`);
    await p.click('.menu [data-vm="gauges"]');
    await until(() => document.querySelectorAll('#stack [data-gw] svg').length >= 3);
    const card = '#stack [data-gw="current_a"]';
    await p.hover(card);
    await p.click(`${card} [data-gwt]`);
    check('a widget without inputs has no Inputs entry', !(await p.$('.widgetmenu [data-wt="inputs"]')));
    await p.click('.widgetmenu [data-wt="yourlist"]');
    await p.click('.widgetmenu [data-wt="cw_tilt"]');
    await until(() => !!document.querySelector(`${card} iframe`));
    await p.hover(card);
    await p.click(`${card} [data-gwt]`);
    check('a code widget with inputs offers Inputs and settings…', !!(await p.$('.widgetmenu [data-wt="inputs"]')));
    await p.click('.widgetmenu [data-wt="inputs"]');
    check('the dialog lists its roles and parameters', await until(() => !!document.querySelector('#widgetInputs [data-inrole="roll"]') && !!document.querySelector('#widgetInputs [data-inrole="pitch"]') && !!document.querySelector('#widgetInputs [data-inparam="gain"]')));
    check('a role starts on the widget\'s own channel', (await p.inputValue('#widgetInputs [data-inrole="roll"]')) === 'current_a');
    check('a parameter shows its default', (await p.inputValue('#widgetInputs [data-inparam="gain"]')) === '2' && (await p.inputValue('#widgetInputs [data-inparam="mode"]')) === 'fast');
    await p.selectOption('#widgetInputs [data-inrole="roll"]', 'battery_v');
    await p.fill('#widgetInputs [data-inparam="gain"]', '5');
    await p.dispatchEvent('#widgetInputs [data-inparam="gain"]', 'change');
    await p.selectOption('#widgetInputs [data-inparam="mode"]', 'slow');
    const kept = async () => {
      const L = (await settingsLS()).layouts ?? {};
      return Object.values(L).flat().map((pn) => pn.widgets?.current_a).find((w) => w?.inputs);
    };
    check('the bindings and values are kept with the pane', await until(async () => true) && (await (async () => { for (let k = 0; k < 20; k++) { const w = await kept(); if (w?.inputs?.roll === 'battery_v' && w?.params?.gain === '5' && w?.params?.mode === 'slow') return true; await p.waitForTimeout(150); } return false; })()), await kept());
    await p.keyboard.press('Escape');
  },
  {
    files: { '/data/flight.tlm': { channels: ['battery_v', 'current_a', 'throttle_pct'], units: ['V', 'A', '%'] } },
    launch: '/data/flight.tlm',
    settings: { customWidgets: [TILT] },
  },
);
