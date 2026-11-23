// What the installed app does for the system around it (R4): files the OS asks it to open (a double-click on a .tlm,
// Open With, a second launch), those asked for while it started and those that come later; and Help › Install
// command-line tool…, which says where the command went and, when its folder isn't on PATH, offers the line to add;
// Help › User manual and Online manual.
import { run } from './lib.js';

const files = { '/data/a.tlm': {}, '/data/b.tlm': {}, '/data/c.tlm': {} };

run(
  'osopen',
  async ({ p, check, fake, until }) => {
    const tabNames = () => p.evaluate(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].map((n) => n.textContent));
    check('a file the OS asked for while the app started opens', await until(() => [...document.querySelectorAll('#wsTabs .wstab .nm')].some((n) => n.textContent === 'b.tlm')), await tabNames());
    check('and the core was asked once for what waited', (await fake(() => window.__tlmFake.osAsked.length)) === 0);

    await fake(() => window.__tlmFake.osOpen(['/data/c.tlm']));
    check('one asked for while it runs opens in front', await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'c.tlm'), await tabNames());
    await fake(() => window.__tlmFake.osOpen(['/data/b.tlm']));
    check('a file already open comes to the front, no second tab', (await until(() => document.querySelector('#wsTabs [aria-selected=true] .nm')?.textContent === 'b.tlm')) && (await tabNames()).filter((n) => n === 'b.tlm').length === 1, await tabNames());

    // Help › Install command-line tool…
    const table = await p.evaluate(() => window.__tlmApp.commandInfo().commands);
    check('Help has Install command-line tool…', table.some((c) => c.id === 'installCli' && c.menu === 'Help' && c.label === 'Install command-line tool…'));
    await fake(() => window.__tlmFake.menuClick('installCli'));
    check('it says where the command went and that its folder is not on PATH', await until(() => /Installed as \/Users\/me\/\.local\/bin\/tlmbench\. Its folder isn't on your PATH yet: add this line to ~\/\.zprofile/.test(document.querySelector('#wsNote')?.textContent ?? '')), await p.evaluate(() => document.querySelector('#wsNote')?.textContent));
    check('with the line, and Copy the line', /export PATH="\$HOME\/\.local\/bin:\$PATH"/.test(await p.evaluate(() => document.querySelector('#wsNote')?.textContent ?? '')) && (await p.evaluate(() => document.querySelector('#wsNote button')?.textContent?.trim())) === 'Copy the line');
    await fake(() => (window.__tlmFake.cliResult = { at: '/Users/me/.local/bin/tlmbench', onPath: true, pathLine: null, profile: null }));
    await fake(() => window.__tlmFake.menuClick('installCli'));
    check('on PATH already: one line, nothing to copy', await until(() => /Installed: a new terminal runs tlmbench/.test(document.querySelector('#wsNote')?.textContent ?? '')));
    await fake(() => (window.__tlmFake.cliError = 'This build has no command-line tool beside the app (a development build).'));
    await fake(() => window.__tlmFake.menuClick('installCli'));
    check('a build without the tool says so', await until(() => /Couldn't install the command-line tool: This build has no command-line tool/.test(document.querySelector('#wsNote')?.textContent ?? '')));

    // Help › User manual (the bundled PDF) and Online manual (R7, decision 6)
    check('Help has User manual and Online manual', table.some((c) => c.id === 'manual' && c.menu === 'Help' && c.label === 'User manual') && table.some((c) => c.id === 'manualOnline' && c.menu === 'Help' && c.label === 'Online manual'));
    await fake(() => window.__tlmFake.menuClick('manual'));
    await fake(() => window.__tlmFake.menuClick('manualOnline'));
    check('they ask the core to open the PDF, then the web copy', await until(() => JSON.stringify(window.__tlmFake.manualOpens) === '[false,true]'), await fake(() => window.__tlmFake.manualOpens));
    await fake(() => (window.__tlmFake.manualError = 'This build has no manual beside the app (a development build).'));
    await fake(() => window.__tlmFake.menuClick('manual'));
    check('a build without the PDF says so', await until(() => /Couldn't open the manual: This build has no manual/.test(document.querySelector('#wsNote')?.textContent ?? '')));
  },
  { files, launch: '/data/a.tlm', setup: (f) => (f.osAsked = ['/data/b.tlm']) },
);
