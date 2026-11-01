// Sharing and export: rule sets, events, categories and recipes as TOML for .tlmbench/; Export to HTML (Full or Light by budget); no "sessions alongside the data".
const T = require('./lib');
T.run('share', async ({ p, check, wait, text, settingsLS }) => {
  // rules & checks: Share… shows the TOML that goes to .tlmbench/checks/
  await p.evaluate(() => document.querySelector('[data-rail="events"]').click()); await wait(300);
  await p.click('#evRules'); await wait(500);
  await p.click('.chkdlg [data-sshare]'); await wait(300);
  const sh = await p.evaluate(() => ({ paths: [...document.querySelectorAll('#recDlg .shpath code')].map(e => e.textContent), toml: document.querySelector('#recDlg .shtxt').value }));
  check('Share… on a set previews its file in .tlmbench/checks/ (and the categories its rules use)', /\.tlmbench\/checks\/event-rules\.toml$/.test(sh.paths[0]) && sh.paths.some(x => /categories\.toml$/.test(x)), sh.paths);
  check('...as TOML: one [[check]] table per row', /^schema = 1/m.test(sh.toml) && /\[\[check\]\]\nname = "Shock"\nkind = "rule"/.test(sh.toml), sh.toml.slice(0, 200));
  await p.click('#recDlg [data-shgo]'); await wait(300);
  check('Share to folder records it (simulated)', /Written: \.tlmbench\/checks\/event-rules\.toml/.test(await text('#recDlg [data-shmsg]')) && Object.keys((await settingsLS()).sharedFiles || {}).length >= 2);
  await p.evaluate(() => document.querySelector('#recDlg [data-x]').click()); await wait(200);
  // events: your markers and the categories
  await p.evaluate(() => document.getElementById('evShare').click()); await wait(300);
  const ev = await p.evaluate(() => [...document.querySelectorAll('#recDlg .shpath code')].map(e => e.textContent));
  check('Share… in Events writes the recording\'s events and the categories', ev.some(x => /\.tlmbench\/events\/synthetic-flight\.toml$/.test(x)) && ev.some(x => /categories\.toml$/.test(x)), ev);
  await p.evaluate(() => document.querySelector('#recDlg [data-x]').click()); await wait(200);
  // recipes round-trip through TOML
  const rt = await p.evaluate(() => { const D = window.__tlmToml; return D ? D.roundtrip() : null; });
  check('recipes export as TOML and import back the same', rt && rt.ok, rt);
  // Export to HTML from the ⋯ menu
  await p.click('#b-more'); await wait(200);
  const ok = await p.evaluate(() => { const b = [...document.querySelectorAll('.moremenu .mi, .moremenu button')].find(x => /Export to HTML/.test(x.textContent)); if (b) b.click(); return !!b; }); await wait(400);
  check('Export to HTML… in the ⋯ menu', ok);
  const ex = await p.evaluate(() => ({ sum: document.querySelector('#recDlg .exsum').innerText, files: [...document.querySelectorAll('#recDlg .exfiles code')].map(e => e.textContent) }));
  check('it says Full or Light and the size, against the budget', /^(Full|Light) · about [\d.]+ (KB|MB|GB)/.test(ex.sum), ex.sum);
  check('...and lists the static files, with a manifest', ex.files.some(f => /index\.html$/.test(f)) && ex.files.some(f => /manifest\.json$/.test(f)) && ex.files.some(f => /data\/.+\.arrow$/.test(f)), ex.files);
  await p.selectOption('#recDlg [data-eo="single"]', '1'); await wait(300);
  check('one .html file has the smaller budget', /25 MB/.test(await text('#recDlg [data-eo="single"]')) && /--single-file/.test(await text('#recDlg .recbody')));
  // sessions stay in the app cache: no setting for "alongside the data"
  check('no "Save sessions in" setting', await p.evaluate(() => !document.querySelector('[data-set="sessionLocation"]')));
});
