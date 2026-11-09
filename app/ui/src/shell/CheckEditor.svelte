<!-- The editor for rules and checks: one set at a time, a row each. A row is a Check (Expect, optionally When; pass or
     fail) or a Rule (Mark when; events in a category). Expect, Mark when and When can be Lists (every sample of · channel
     · > · value) or ƒ, an expression with autocomplete; both edit the same saved text. Each row previews on the file in
     front. The core runs everything; nothing is written beside the data. -->
<script lang="ts">
  import { duration, readoutTime } from '../axes';
  import { buildPick, type Agg, type Op, type Pick, parsePick } from '../checkexpr';
  import { type Check, checks, newCheck, type Outcome } from '../checks.svelte';
  import { FUNCTION_NAMES, quoteName, recipes } from '../derive.svelte';
  import { events } from '../events.svelte';
  import { ICON } from '../icons';
  import { limitsOf } from '../display.svelte';
  import { setSetting, settings } from '../settings.svelte';
  import { tabs } from '../tabs.svelte';
  import { actions, ui } from '../ui.svelte';

  const ed = $derived(checks.editor);
  const layout = $derived(tabs.list.find((t) => t.key === tabs.active)?.ws ?? null);
  let error = $state('');
  let saving = $state(false);

  // ----- rows -----
  /** Rows shown as an expression (Expect/Mark when and When), by row and field; the rest read as Lists when they can. */
  let asText = $state<Record<string, boolean>>({});
  const mode = (i: number, f: 'expect' | 'when') => asText[`${i}.${f}`] ?? parsePick((checks.editor?.set.check[i] as Check)[f] ?? '') == null;
  function setMode(i: number, f: 'expect' | 'when', text: boolean) {
    asText[`${i}.${f}`] = text;
    if (!text && parsePick(ed!.set.check[i][f] ?? '') == null) {
      if (f === 'when') ed!.set.check[i].when = undefined;
      else ed!.set.check[i].expect = '';
    }
  }
  const channels = $derived(ui.channels.map((c) => c.name));
  const pickOf = (text: string | undefined): Pick => parsePick(text ?? '') ?? { agg: 'each', ch: channels[0] ?? '', op: '>', v: '0', v2: '' };
  function edit(i: number, f: 'expect' | 'when', patch: Partial<Pick>) {
    const p = { ...pickOf(ed!.set.check[i][f]), ...patch };
    // a channel with value names offers is / is not instead of a number
    if ((p.op === 'is' || p.op === 'isnt') && !p.v) p.v = Object.values(settings.valueMaps[p.ch] ?? {})[0] ?? '';
    if ((p.op === 'warn' || p.op === 'alarm') && patch.op) p.agg = 'each';
    if (p.op === 'outside' && !p.v2) p.v2 = String(+p.v + 1);
    ed!.set.check[i][f] = buildPick(p);
  }
  const labels = (ch: string) => Object.values(settings.valueMaps[ch] ?? {});
  const limitsText = (ch: string, kind: 'warn' | 'alarm') => {
    const l = limitsOf(ch);
    const [lo, hi] = kind === 'warn' ? [l.wl, l.wh] : [l.al, l.ah];
    return lo == null && hi == null ? 'no limits set' : `${lo ?? '…'} to ${hi ?? '…'}`;
  };

  function addRow(rule: boolean) {
    if (!ed) return;
    const last = ed.set.check.at(-1);
    const row = newCheck(rule, last?.group ?? (rule ? 'Alarm' : 'Checks'));
    if (channels[0]) row.expect = buildPick({ agg: 'each', ch: channels[0], op: '>', v: '0', v2: '' });
    ed.set.check.push(row);
    ed.focus = ed.set.check.length - 1;
  }
  function fromLimits() {
    if (!ed) return;
    let n = 0;
    for (const c of ui.channels) {
      const l = limitsOf(c.name);
      const q = quoteName(c.name);
      for (const [lo, hi, word] of [[l.al, l.ah, 'alarm limits']] as const) {
        if (lo == null && hi == null) continue;
        const expect = lo != null && hi != null ? `${q} > ${lo} && ${q} < ${hi}` : hi != null ? `${q} < ${hi}` : `${q} > ${lo}`;
        if (ed.set.check.some((k) => k.expect === expect)) continue;
        ed.set.check.push({ ...newCheck(false, 'Limits'), name: `${c.name} within ${word}`, expect });
        n++;
      }
    }
    error = n ? '' : 'No channel in this file has alarm limits to check against.';
  }
  const move = (i: number, d: number) => {
    const rows = ed!.set.check;
    if (i + d < 0 || i + d >= rows.length) return;
    [rows[i], rows[i + d]] = [rows[i + d], rows[i]];
    sigs = {};
    asText = {};
  };
  function remove(i: number) {
    ed!.set.check.splice(i, 1);
    sigs = {};
    asText = {};
  }
  const groups = $derived([...new Set(ed?.set.check.map((k) => k.group) ?? [])]);

  // ----- categories -----
  function newCategory(i: number): void {
    const name = window.prompt('Name the new category')?.trim();
    if (!name) return void (ed!.set.check[i].category = ed!.set.check[i].category ?? 'alarm');
    const id = settings.eventCats.find((c) => c.name.toLowerCase() === name.toLowerCase())?.id ?? addCategory(name);
    ed!.set.check[i].category = id;
  }
  function addCategory(name: string): string {
    const colors = ['#FF5A6E', '#FFB547', '#9D8CFF', '#F25FC0', '#3FCF8E', '#4DA3FF', '#2BD4C4'];
    const id = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'cat' + Date.now().toString(36);
    const uniq = settings.eventCats.some((c) => c.id === id) ? id + '-' + Date.now().toString(36) : id;
    setSetting('eventCats', [...settings.eventCats, { id: uniq, name, color: colors[settings.eventCats.length % colors.length], chart: true }]);
    return uniq;
  }
  const catStats = (id: string) => ({ events: events.list.filter((e) => e.category === id).length, rules: ed!.set.check.filter((k) => k.kind === 'rule' && k.category === id).length });
  function renameCat(i: number, name: string) {
    if (!name.trim()) return;
    setSetting('eventCats', settings.eventCats.map((c, j) => (j === i ? { ...c, name: name.trim() } : c)));
  }
  function plusCategory() {
    addCategory('New category');
    checks.editor!.cats = true;
    queueMicrotask(() => {
      const all = document.querySelectorAll<HTMLInputElement>('[data-cname]');
      const last = all[all.length - 1];
      last?.focus();
      last?.select();
    });
  }

  // ----- attaching -----
  const attachedFile = $derived(ed && ui.path ? (settings.checkAttach.files[ui.path] ?? []).includes(ed.was ?? ed.set.name) : false);
  const attachedLayout = $derived(ed && layout ? (settings.checkAttach.layouts[layout] ?? []).includes(ed.was ?? ed.set.name) : false);
  function attach(kind: 'files' | 'layouts', key: string, on: boolean) {
    const name = ed!.was ?? ed!.set.name;
    const A = settings.checkAttach;
    const cur = A[kind][key] ?? [];
    setSetting('checkAttach', { ...A, [kind]: { ...A[kind], [key]: on ? [...new Set([...cur, name])] : cur.filter((n) => n !== name) } });
  }

  // ----- preview on the file in front -----
  let sigs = $state<Record<number, string>>({});
  let prev = $state<Record<number, string>>({});
  const at = (ns: string) => readoutTime(BigInt(ns), Math.max(1, Number(BigInt(ui.view.t1) - BigInt(ui.view.t0))));
  const fileName = $derived(ui.file);
  function words(o: Outcome, rule: boolean): string {
    const num = (x: number) => (Math.abs(x) >= 1000 || Number.isInteger(x) ? x.toFixed(0) : String(+x.toFixed(3)));
    switch (o.verdict) {
      case 'off': return 'turned off';
      case 'err': case 'cant': case 'na': return o.message ?? '';
      case 'mark': return o.count ? `marks ${o.count} event${o.count === 1 ? '' : 's'} · ${duration(o.secs * 1e9)} in all` : 'no events';
      case 'pass': return o.value != null && o.function ? `passes · ${o.function} ${num(o.value)}${o.limit != null ? ` (limit ${num(o.limit)})` : ''}` : 'passes';
      default: {
        if (o.value != null && o.function) return `${o.verdict === 'warn' ? 'warns' : 'fails'} · ${o.function} ${num(o.value)}${o.limit != null ? ` (limit ${num(o.limit)})` : ''}`;
        return [o.verdict === 'warn' ? 'warns' : 'fails', `${o.count}×`, duration(o.secs * 1e9), o.worst != null && o.worst_t ? `worst ${num(o.worst)}` : '', o.runs[0] ? `first at ${at(o.runs[0].start)}` : ''].filter(Boolean).join(' · ');
      }
    }
    void rule;
  }
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    if (!ed) return;
    const rows = $state.snapshot(ed.set.check);
    const sig = rows.map((k) => JSON.stringify([k.expect, k.when, k.grace, k.settle, k.severity, k.kind, k.enabled]));
    clearTimeout(timer);
    timer = setTimeout(async () => {
      for (let i = 0; i < rows.length; i++) {
        if (sigs[i] === sig[i]) continue;
        sigs[i] = sig[i];
        const o = await actions.checkPreview(rows[i]);
        if (o && checks.editor) prev[i] = `On ${fileName}: ${words(o, rows[i].kind === 'rule')}`;
      }
    }, 350);
    return () => clearTimeout(timer);
  });

  // ----- autocomplete for expressions -----
  let sugg = $state<{ field: string; items: string[]; at: number; from: number; pick: number } | null>(null);
  function suggest(i: number, f: 'expect' | 'when', el: HTMLInputElement) {
    const text = el.value;
    const c = el.selectionStart ?? text.length;
    const q = /("[^"]+"|[A-Za-z_][A-Za-z0-9_.]*)\s*[=!]=\s*'([^']*)$/.exec(text.slice(0, c));
    if (q) {
      const typed = q[2].toLowerCase();
      const items = labels(q[1].replace(/^"|"$/g, '')).filter((l) => l.toLowerCase().startsWith(typed) && l.toLowerCase() !== typed).map((l) => l + "'").slice(0, 8);
      sugg = items.length ? { field: `${i}.${f}`, items, at: c, from: c - q[2].length, pick: 0 } : null;
      return;
    }
    const m = /[A-Za-z_][A-Za-z0-9_.]*$/.exec(text.slice(0, c));
    if (!m) return void (sugg = null);
    const t = m[0].toLowerCase();
    const names = [...ui.channels.map((x) => (x.derived ? `${quoteName(x.name)} ƒ` : quoteName(x.name))), ...recipes.list.map((r) => `${quoteName(r.name)} ƒ`)];
    const all = [...names, ...FUNCTION_NAMES.map((n) => `${n}(`)];
    const items = [...new Set(all)].filter((s) => s.replace(/^"/, '').toLowerCase().startsWith(t) && s.replace(/^"/, '').toLowerCase() !== t).slice(0, 8);
    sugg = items.length ? { field: `${i}.${f}`, items, at: c, from: c - m[0].length, pick: 0 } : null;
  }
  function accept(i: number, f: 'expect' | 'when', el: HTMLInputElement, item: string) {
    const s = item.replace(/ ƒ$/, '');
    const text = el.value.slice(0, sugg!.from) + s + el.value.slice(sugg!.at);
    const at = sugg!.from + s.length;
    ed!.set.check[i][f] = text;
    el.value = text;
    sugg = null;
    queueMicrotask(() => (el.setSelectionRange(at, at), el.focus()));
  }
  function complete(e: KeyboardEvent, i: number, f: 'expect' | 'when') {
    if (!sugg || sugg.field !== `${i}.${f}`) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      sugg.pick = (sugg.pick + (e.key === 'ArrowDown' ? 1 : sugg.items.length - 1)) % sugg.items.length;
    } else if (e.key === 'Tab' || e.key === 'Enter') {
      e.preventDefault();
      accept(i, f, e.currentTarget as HTMLInputElement, sugg.items[sugg.pick]);
    } else if (e.key === 'Escape') {
      e.stopPropagation();
      sugg = null;
    }
  }

  // ----- save, close -----
  async function save() {
    if (!ed || saving) return;
    saving = true;
    const err = await actions.checkSave($state.snapshot(ed.set), ed.was);
    saving = false;
    if (err) error = err;
    else close();
  }
  function close() {
    checks.editor = null;
    error = '';
    sugg = null;
    asText = {};
    sigs = {};
    prev = {};
  }
  function remset() {
    if (!ed?.was) return close();
    if (!window.confirm(`Delete the set ${ed.was}?`)) return;
    actions.checkDelete(ed.was);
    close();
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape' && !sugg) {
      e.stopPropagation();
      close();
    }
  }
  $effect(() => {
    if (!ed || ed.focus < 0) return;
    const f = ed.focus;
    queueMicrotask(() => document.querySelector<HTMLElement>(`.ckrow[data-k="${f}"]`)?.scrollIntoView({ block: 'nearest' }));
  });
  const OPS: [Op, string][] = [['<', '<'], ['<=', '≤'], ['>', '>'], ['>=', '≥'], ['==', '='], ['!=', '≠']];
  const AGGS: [Agg, string][] = [['each', 'every sample of'], ['max', 'the max of'], ['min', 'the min of'], ['mean', 'the mean of'], ['abs', 'the magnitude of']];
</script>

<svelte:window onkeydowncapture={(e) => ed && key(e)} />

{#if ed}
  <div class="scrim" role="presentation" onclick={close}></div>
  <div class="dlg recdlg ckdlg" id="recDlg" role="dialog" aria-label="Rules and checks">
    <header><b>{ed.was ? 'Edit set' : 'New set'}</b><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={close}>{@html ICON.close}</button></header>
    <div class="recbody ckbody">
      <div class="cktop">
        <label class="lvf"><span>Set name</span><input class="sel" data-sname bind:value={ed.set.name} spellcheck="false" /></label>
        <div class="ckatt">
          <label class="radio"><input type="checkbox" data-sany bind:checked={ed.set.runs_on_any_file} /><span>Run on every file with these channels</span></label>
          {#if !ed.set.runs_on_any_file}
            {#if ui.path}<label class="radio"><input type="checkbox" data-sfile checked={attachedFile} onchange={(e) => attach('files', ui.path, e.currentTarget.checked)} /><span>This file</span></label>{/if}
            {#if layout}<label class="radio"><input type="checkbox" data-slayout checked={attachedLayout} onchange={(e) => attach('layouts', layout, e.currentTarget.checked)} /><span>Whenever the layout {layout} is in front</span></label>{/if}
          {/if}
          <span class="hint">Runs where it's attached; a check whose channel is missing says "can't run".</span>
        </div>
      </div>

      {#each ed.set.check as row, i (i)}
        <div class="ckrow" class:focus={ed.focus === i} class:off={!row.enabled} data-k={i}>
          <div class="ckr1">
            <label class="sw" data-tip="Turn this row on or off"><input type="checkbox" data-kf="on" bind:checked={row.enabled} aria-label="On" /></label>
            <select class="sel kind" data-kf="kind" bind:value={row.kind} onchange={() => (row.kind === 'rule' ? ((row.category ??= 'alarm'), (row.severity = 'fail')) : (row.category = undefined))}>
              <option value="check">Check</option><option value="rule">Rule</option>
            </select>
            <input class="sel nm" data-kf="name" placeholder="Name" bind:value={row.name} spellcheck="false" />
            <input class="sel grp" data-kf="group" list="ckgroups" placeholder="Group" data-tip="A group takes its worst result. A new name starts a group." bind:value={row.group} spellcheck="false" />
            {#if row.kind === 'check'}
              <select class="sel" data-kf="sev" bind:value={row.severity} aria-label="Severity"><option value="fail">Fail</option><option value="warn">Warn</option></select>
            {:else}
              <select class="sel" data-kf="cat" aria-label="Event category" value={row.category ?? ''} onchange={(e) => (e.currentTarget.value === '__new' ? newCategory(i) : (row.category = e.currentTarget.value))}>
                {#each settings.eventCats as c (c.id)}<option value={c.id}>{c.name}</option>{/each}<option value="__new">+ New category…</option>
              </select>
            {/if}
            <button class="ibtn" aria-label="Move up" data-tip="Move up" disabled={i === 0} onclick={() => move(i, -1)}>▲</button>
            <button class="ibtn" aria-label="Move down" data-tip="Move down" disabled={i === ed.set.check.length - 1} onclick={() => move(i, 1)}>▼</button>
            <button class="ibtn" data-kdel aria-label="Delete this row" data-tip="Delete this row" onclick={() => remove(i)}>{@html ICON.trash}</button>
          </div>

          {#each ['expect', 'when'] as const as f (f)}
            {#if f === 'expect' || row.when !== undefined}
              <div class="ckr2">
                <span class="cklab">{f === 'when' ? 'When' : row.kind === 'rule' ? 'Mark when' : 'Expect'}</span>
                <div class="ckx">
                  {#if !mode(i, f)}
                    {@const p = pickOf(row[f])}
                    {#if p.op !== 'warn' && p.op !== 'alarm' && p.op !== 'outside' && p.op !== 'is' && p.op !== 'isnt'}
                      <select class="sel" data-xp="agg" value={p.agg} onchange={(e) => edit(i, f, { agg: e.currentTarget.value as Agg })}>{#each AGGS as [v, t] (v)}<option value={v}>{t}</option>{/each}</select>
                    {/if}
                    <select class="sel" data-xp="ch" value={p.ch} onchange={(e) => edit(i, f, { ch: e.currentTarget.value })}>{#each channels as c (c)}<option value={c}>{c}</option>{/each}</select>
                    <select class="sel" data-xp="op" value={p.op} onchange={(e) => edit(i, f, { op: e.currentTarget.value as Op })}>
                      {#each OPS as [v, t] (v)}<option value={v}>{t}</option>{/each}
                      {#if p.agg === 'each'}
                        <option value="outside">outside</option>
                        {#if labels(p.ch).length}<option value="is">is</option><option value="isnt">is not</option>{/if}
                        <option value="warn">past its warn limits</option><option value="alarm">past its alarm limits</option>
                      {/if}
                    </select>
                    {#if p.op === 'is' || p.op === 'isnt'}
                      <select class="sel" data-xp="v" value={p.v} onchange={(e) => edit(i, f, { v: e.currentTarget.value })}>{#each labels(p.ch) as l (l)}<option value={l}>{l}</option>{/each}</select>
                    {:else if p.op === 'warn' || p.op === 'alarm'}
                      <span class="muted">({limitsText(p.ch, p.op)})</span>
                    {:else if p.op === 'outside'}
                      <input class="sel num" data-xp="v" type="number" step="any" value={p.v} onchange={(e) => edit(i, f, { v: e.currentTarget.value })} /><span class="muted">and</span>
                      <input class="sel num" data-xp="v2" type="number" step="any" value={p.v2} onchange={(e) => edit(i, f, { v2: e.currentTarget.value })} />
                    {:else}
                      <input class="sel num" data-xp="v" type="number" step="any" value={p.v} onchange={(e) => edit(i, f, { v: e.currentTarget.value })} />
                    {/if}
                  {:else}
                    <div class="xw">
                      <input class="sel expr" data-xt={f} value={row[f] ?? ''} spellcheck="false" placeholder={f === 'when' ? "flight_mode == 'Mission'" : 'motor_rpm < 7500'} oninput={(e) => ((row[f] = e.currentTarget.value), suggest(i, f, e.currentTarget))} onkeydown={(e) => complete(e, i, f)} onblur={() => setTimeout(() => (sugg = null), 150)} />
                      {#if sugg && sugg.field === `${i}.${f}`}
                        <ul class="acpop" role="listbox">{#each sugg.items as s, k (s)}<li class="aci" class:on={k === sugg.pick} role="option" aria-selected={k === sugg.pick}><button onmousedown={(e) => (e.preventDefault(), accept(i, f, (e.currentTarget as HTMLElement).closest('.xw')!.querySelector('input') as HTMLInputElement, s))}><span>{s.replace(/ ƒ$/, '')}</span>{#if s.endsWith(' ƒ')}<small>ƒ</small>{/if}</button></li>{/each}</ul>
                      {/if}
                    </div>
                  {/if}
                  <button class="linkbtn" data-xm={f} data-tip={mode(i, f) ? 'Back to Lists, when the expression is simple' : 'Write it as an expression'} onclick={() => setMode(i, f, !mode(i, f))}>{mode(i, f) ? 'Lists' : 'ƒ'}</button>
                  {#if f === 'when'}<button class="ibtn" data-xw aria-label="Remove When" data-tip="Remove When" onclick={() => (row.when = undefined)}>{@html ICON.close}</button>{/if}
                </div>
              </div>
            {/if}
          {/each}

          <div class="ckr3">
            {#if row.when === undefined}<button class="linkbtn" data-kwhen onclick={() => ((row.when = ''), (asText[`${i}.when`] = true))}>+ When</button>{/if}
            <label class="lvg">Grace <input class="sel num" data-kf="grace" type="number" min="0" step="any" bind:value={row.grace} /> s</label>
            {#if row.when !== undefined}<label class="lvg">Settle <input class="sel num" data-kf="settle" type="number" min="0" step="any" bind:value={row.settle} /> s</label>{/if}
            <span class="kp muted" data-kp>{prev[i] ?? 'Checking on the open file…'}</span>
          </div>
        </div>
      {/each}
      <datalist id="ckgroups">{#each groups as g (g)}<option value={g}></option>{/each}</datalist>

      <div class="ckadd">
        <button class="tbtn" data-kadd onclick={() => addRow(false)}>{@html ICON.plus}Check</button>
        <button class="tbtn" data-kaddr onclick={() => addRow(true)}>{@html ICON.plus}Rule</button>
        <button class="tbtn" data-kfromlim data-tip="Add a check for each alarm limit in the file" onclick={fromLimits}>+ From limits</button>
      </div>

      {#if groups.length}
        <div class="ckinfo"><span class="muted">Info only (shown, never fails the set):</span>
          {#each groups as g (g)}<label class="radio"><input type="checkbox" data-kinfo={g} checked={ed.set.info_groups.includes(g)} onchange={(e) => (ed!.set.info_groups = e.currentTarget.checked ? [...ed!.set.info_groups, g] : ed!.set.info_groups.filter((x) => x !== g))} /><span>{g}</span></label>{/each}
        </div>
      {/if}

      <details class="ckcats" data-ckcats open={ed.cats} ontoggle={(e) => (checks.editor!.cats = e.currentTarget.open)}>
        <summary>Event categories</summary>
        <div class="ckcatrows">
          {#each settings.eventCats as c, i (c.id)}
            {@const n = catStats(c.id)}
            <div class="ckcat"><i style="background:{c.color}"></i><input class="sel" data-cname={i} value={c.name} spellcheck="false" onchange={(e) => renameCat(i, e.currentTarget.value)} /><span class="muted">{n.events} event{n.events === 1 ? '' : 's'} · {n.rules} rule{n.rules === 1 ? '' : 's'}</span></div>
          {/each}
          <button class="linkbtn" data-cadd onclick={plusCategory}>+ Category</button>
        </div>
      </details>
      {#if error}<p class="recstat bad" role="alert">{error}</p>{/if}
    </div>
    <footer>
      {#if ed.was}<button class="linkbtn" data-sdel onclick={remset}>Delete set</button>{/if}
      <span class="spacer"></span>
      <button class="btn" data-x onclick={close}>Cancel</button>
      <button class="btn primary" data-ssave disabled={saving} onclick={save}>Save</button>
    </footer>
  </div>
{/if}

<style>
  .ckdlg {
    width: min(860px, 94vw);
  }
  .ckbody {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 70vh;
    overflow: auto;
  }
  .cktop {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
  }
  .cktop .lvf {
    flex: 1;
    min-width: 200px;
  }
  .ckatt {
    display: flex;
    flex: 2;
    flex-direction: column;
    gap: 3px;
    min-width: 240px;
    font-size: 12px;
  }
  .hint {
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .lvf {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
  }
  .lvf > span:first-child {
    color: var(--fg-muted);
  }
  .ckrow {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .ckrow.focus {
    border-color: var(--accent);
  }
  .ckrow.off {
    opacity: 0.6;
  }
  .ckr1,
  .ckr2,
  .ckr3,
  .ckx {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .ckr2 {
    flex-wrap: nowrap;
    align-items: flex-start;
  }
  .ckx {
    flex: 1;
    min-width: 0;
  }
  .cklab {
    flex: none;
    width: 74px;
    padding-top: 6px;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .ckrow .sel {
    min-height: 28px;
    padding: 0 6px;
    font-size: 12px;
  }
  .nm {
    flex: 2;
    min-width: 140px;
  }
  .grp {
    flex: 1;
    min-width: 90px;
  }
  .kind {
    width: 78px;
  }
  .num {
    width: 90px;
    font-family: var(--font-data);
  }
  .xw {
    position: relative;
    flex: 1;
    min-width: 220px;
  }
  .expr {
    width: 100%;
    font-family: var(--font-data);
  }
  .acpop {
    position: absolute;
    z-index: 5;
    top: 100%;
    left: 0;
    min-width: 200px;
    margin: 2px 0 0;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    list-style: none;
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.25);
  }
  .aci button {
    display: flex;
    justify-content: space-between;
    width: 100%;
    padding: 3px 8px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg);
    font: 12px var(--font-data);
    text-align: left;
  }
  .aci.on button,
  .aci button:hover {
    background: var(--raised);
  }
  .aci small {
    color: var(--fg-muted);
  }
  .lvg {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
  }
  .kp {
    flex: 1;
    min-width: 160px;
    text-align: right;
    font-size: 11.5px;
  }
  .ckadd,
  .ckinfo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .ckadd :global(svg) {
    width: 11px;
    height: 11px;
    margin-right: 4px;
  }
  .radio {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .ckcats summary {
    color: var(--fg-muted);
    font-size: 12px;
    cursor: pointer;
  }
  .ckcatrows {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 6px;
  }
  .ckcat {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .ckcat i {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .ckcat .sel {
    width: 200px;
  }
  .recstat.bad {
    color: var(--danger);
    font-size: 12px;
  }
</style>
