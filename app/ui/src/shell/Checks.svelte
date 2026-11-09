<!-- The Rules & checks panel: the sets that run on the open file, their groups and rows. A check says what should hold
     and gets a verdict; a rule marks events. Click a row to select it and go to its first violation; ▲ ▼ step through the
     others; double-click (or the pencil) edits it; the channels button opens what it reads. Results come from the core,
     cached by the file's contents and the set's version. -->
<script lang="ts">
  import { duration, readoutTime } from '../axes';
  import { type Check, checks, ICONCH, type Outcome, outcomeText, setsFor, summary, type Verdict, WORD } from '../checks.svelte';
  import { ICON } from '../icons';
  import { endPanelDrag, startPanelDrag } from '../panels.svelte';
  import { movePanel, setPanelOpen, setSetting, settings, sideOf } from '../settings.svelte';
  import { newPaneWith, addChannels, stack } from '../stack.svelte';
  import { tabs } from '../tabs.svelte';
  import { actions, ui } from '../ui.svelte';
  import Menu, { type MenuItem } from './Menu.svelte';

  const closed = $derived(settings.panelsClosed.includes('checks'));
  const other = $derived(sideOf('checks') === 'left' ? 'right' : 'left');
  const layout = $derived(tabs.list.find((t) => t.key === tabs.active)?.ws ?? null);
  const applied = $derived(ui.path ? setsFor(ui.path, layout) : []);
  const idle = $derived(checks.sets.filter((s) => !applied.includes(s)));
  const results = $derived(checks.runs[ui.path] ?? {});
  const sum = $derived(ui.path ? summary(ui.path) : null);
  const span = $derived(Math.max(1, Number(BigInt(ui.view.t1) - BigInt(ui.view.t0))));
  const at = (ns: string) => readoutTime(BigInt(ns), span);
  const secs = (s: number) => duration(s * 1e9);

  const key = (set: string, i: number) => `${set}/${i}`;
  const toggle = (k: string) => (checks.closed = checks.closed.includes(k) ? checks.closed.filter((x) => x !== k) : [...checks.closed, k]);
  /** Which violation each row stepped to. */
  let at_ = $state<Record<string, number>>({});

  function groupText(g: { verdict: Verdict; events: number; total: number; failed: number; warned: number; passed: number }, n: number): string {
    const ev = g.events ? `${g.events} event${g.events === 1 ? '' : 's'}` : '';
    if (g.verdict === 'mark') return ev || 'no events';
    const main =
      g.verdict === 'off' ? 'off'
      : g.verdict === 'na' ? 'not applicable'
      : g.verdict === 'cant' || g.verdict === 'err' ? (g.passed ? `${g.passed} of ${n} ran and passed` : g.verdict === 'err' ? 'error' : "can't run")
      : g.failed || g.warned ? [g.failed ? `${g.failed} of ${g.total} failed` : '', g.warned ? `${g.warned} warning${g.warned > 1 ? 's' : ''}` : ''].filter(Boolean).join(', ')
      : `${g.passed}/${g.total}`;
    return ev ? `${ev} · ${main}` : main;
  }
  /** A set's line: what failed and warned in the groups that count (not the info-only ones). */
  function setText(r: { verdict: Verdict; events: number; groups: { info: boolean; failed: number; warned: number }[] }): string {
    if (r.verdict === 'mark') return `${r.events} event${r.events === 1 ? '' : 's'}`;
    const failed = r.groups.reduce((n, g) => n + (g.info ? 0 : g.failed), 0);
    const warned = r.groups.reduce((n, g) => n + (g.info ? 0 : g.warned), 0);
    return failed || warned ? [failed ? `${failed} failed` : '', warned ? `${warned} warning${warned > 1 ? 's' : ''}` : ''].filter(Boolean).join(', ') : WORD[r.verdict].toLowerCase();
  }
  const tip = (k: Check, o: Outcome | undefined) => {
    const t = [`Expect ${k.expect}`];
    if (k.when) t.push(`when ${k.when}`);
    if (k.grace) t.push(`grace ${k.grace} s`);
    if (k.settle) t.push(`settle ${k.settle} s`);
    if (o?.channels.length) t.push(`channels: ${o.channels.join(', ')}`);
    return t.join(' · ');
  };

  function go(set: string, i: number, o: Outcome | undefined, n: number) {
    if (!o?.runs.length) return;
    const k = key(set, i);
    const m = (n + o.runs.length) % o.runs.length;
    at_[k] = m;
    actions.checkShow(o.runs[m].start, o.runs[m].end);
  }
  function pick(set: string, i: number, o: Outcome | undefined) {
    const k = key(set, i);
    if (checks.sel === k) return void (checks.sel = null);
    checks.sel = k;
    go(set, i, o, 0);
  }
  /** The channel numbers of the open file a row reads. */
  const chans = (o: Outcome | undefined) => (o?.channels ?? []).map((n) => ui.channels.findIndex((c) => c.name === n)).filter((i) => i >= 0);

  let menu = $state<null | { anchor?: HTMLElement; at?: { x: number; y: number }; items: MenuItem[]; label: string }>(null);
  function chanMenu(e: MouseEvent, o: Outcome | undefined) {
    const chs = chans(o);
    const n = chs.length;
    menu = {
      anchor: e.currentTarget as HTMLElement,
      label: 'Open channels',
      items: [
        { label: `Open its ${n} channel${n === 1 ? '' : 's'} in a new chart`, run: () => newPaneWith(chs) },
        { label: `Add its ${n} channel${n === 1 ? '' : 's'} to the selected pane`, disabled: !stack.panes.length, run: () => addChannels(chs) },
        { label: `Open its ${n} channel${n === 1 ? '' : 's'} in a new table`, run: () => newPaneWith(chs, 'table') },
      ],
    };
  }
  function rowMenu(e: MouseEvent, set: string, i: number, o: Outcome | undefined) {
    e.preventDefault();
    const chs = chans(o);
    const n = chs.length;
    menu = {
      at: { x: e.clientX, y: e.clientY },
      label: 'Check',
      items: [
        { label: 'Edit this check', run: () => actions.checkEditor(set, i) },
        { label: checks.sel === key(set, i) ? 'Deselect' : 'Select', run: () => (checks.sel = checks.sel === key(set, i) ? null : key(set, i)) },
        { sep: true },
        { label: `Open its ${n} channel${n === 1 ? '' : 's'} in a new chart`, disabled: !n, run: () => newPaneWith(chs) },
        { label: `Add its ${n} channel${n === 1 ? '' : 's'} to the selected pane`, disabled: !n || !stack.panes.length, run: () => addChannels(chs) },
      ],
    };
  }
  function attach(set: string) {
    const A = settings.checkAttach;
    setSetting('checkAttach', { ...A, files: { ...A.files, [ui.path]: [...new Set([...(A.files[ui.path] ?? []), set])] } });
  }
  function detach(set: string) {
    const A = settings.checkAttach;
    setSetting('checkAttach', { ...A, files: { ...A.files, [ui.path]: (A.files[ui.path] ?? []).filter((n) => n !== set) } });
  }
  function bg(e: MouseEvent) {
    if (!(e.target as HTMLElement).closest('.crow, button, .cset')) checks.sel = null;
  }
  function esc(e: KeyboardEvent) {
    if (e.key === 'Escape' && checks.sel && !menu) checks.sel = null;
  }
</script>

<svelte:window onkeydown={esc} />

<div class="ckin" id="checksPanel">
  <div class="phd" role="presentation" draggable="true" ondragstart={(e) => startPanelDrag(e, 'checks')} ondragend={endPanelDrag}>
    <button class="phtog" aria-expanded={!closed} data-tip="Collapse or expand. Drag the title to move this panel." onclick={() => setPanelOpen('checks', closed)}><span class="chv" class:shut={closed}>{@html ICON.chev}</span>Rules & checks</button>
    {#if sum && !closed}<span class="cbadge" class:bad={sum.failed > 0} data-cn="{sum.failed ? '✗ ' + sum.failed : sum.warned ? '! ' + sum.warned : '✓ ' + sum.passed}">{sum.failed ? `✗ ${sum.failed}` : sum.warned ? `! ${sum.warned}` : `✓ ${sum.passed}`}</span>{/if}
    <span class="spacer"></span>
    <button class="ibtn pmove" data-pmove aria-label="Move Rules & checks to the {other} sidebar" data-tip="Move to the {other} sidebar" onclick={() => movePanel('checks', other)}>{@html ICON.swap}</button>
  </div>
  {#if !closed}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div id="checksBody" class="ckbody" onclick={bg}>
      <div class="ckbar">
        <button class="tbtn" data-cnew onclick={() => actions.checkEditor(null)}>{@html ICON.plus}New set</button>
        <button class="tbtn" data-cnewrule onclick={() => actions.checkEditor(null, -1, true)}>{@html ICON.plus}Rule</button>
        <button class="tbtn" data-cfolder disabled={!ui.path} data-tip="Run a set on every recording in a folder" onclick={() => (checks.folder = true)}>Run on folder…</button>
      </div>
      {#if !ui.path}
        <p class="cknote">Open a recording to run checks on it.</p>
      {:else if !checks.sets.length}
        <p class="cknote">No sets yet. A check says what should hold ("battery_v > 21 while armed"); a rule marks events where a condition holds.</p>
      {/if}
      {#if ui.path && checks.running && !Object.keys(results).length}<p class="cknote" data-cprog>Checking…</p>{/if}
      {#each applied as s (s.name)}
        {@const r = results[s.name]}
        {@const open = !checks.closed.includes(s.name)}
        <div class="cset" data-cset={s.name}>
          <div class="csethd">
            <button class="cshead" aria-expanded={open} onclick={() => toggle(s.name)}><span class="chv" class:shut={!open}>{@html ICON.chev}</span><span class="csn">{s.name}</span></button>
            {#if r}<span class="cv {r.rollup.verdict}" data-cv>{ICONCH[r.rollup.verdict]} <small>{setText(r.rollup)}</small></span>{/if}
            <button class="cedit" data-cedit={s.name} aria-label="Edit {s.name}" data-tip="Edit this set" onclick={() => actions.checkEditor(s.name)}>{@html ICON.edit}</button>
          </div>
          {#if open && r}
            {#each r.rollup.groups as g (g.name)}
              {@const gk = `${s.name}/${g.name}`}
              {@const gopen = g.info ? checks.closed.includes('+' + gk) : !checks.closed.includes(gk)}
              <button class="cgrp" data-cgrp={g.name} aria-expanded={gopen} onclick={() => toggle(g.info ? '+' + gk : gk)}>
                <span class="chv" class:shut={!gopen}>{@html ICON.chev}</span><span class="cv {g.verdict}">{ICONCH[g.verdict]}</span><span class="cgn">{g.name}</span>
                {#if g.info}<span class="muted">info only</span>{/if}
                <span class="muted cgs">{groupText(g, s.check.filter((k) => k.group === g.name).length)}</span>
              </button>
              {#if gopen}
                {#each s.check as k, i (i)}
                  {#if k.group === g.name}
                    {@const o = r.outcomes[i]}
                    {@const kk = key(s.name, i)}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <div class="crow" class:sel={checks.sel === kk} class:off={!k.enabled} role="button" tabindex="0" aria-pressed={checks.sel === kk} data-tip={tip(k, o)} onclick={() => pick(s.name, i, o)} ondblclick={() => actions.checkEditor(s.name, i)} oncontextmenu={(e) => rowMenu(e, s.name, i, o)} onkeydown={(e) => e.key === 'Enter' && pick(s.name, i, o)}>
                      <span class="cv {o?.verdict ?? 'na'}">{k.kind === 'rule' && o?.verdict !== 'off' ? '⚑' : ICONCH[o?.verdict ?? 'na']}</span>
                      <span class="ckn">{k.name}</span>
                      <span class="ckd muted">{o ? outcomeText(o, at, secs) : ''}</span>
                      <span class="ckacts">
                        {#if o?.runs.length}
                          <button class="cstep" data-cprev aria-label="Previous violation" data-tip="Previous" onclick={(e) => (e.stopPropagation(), go(s.name, i, o, (at_[kk] ?? 0) - 1))}>▲</button>
                          <button class="cstep" data-cnext aria-label="Next violation" data-tip="Next" onclick={(e) => (e.stopPropagation(), go(s.name, i, o, (at_[kk] ?? -1) + 1))}>▼</button>
                        {/if}
                        {#if o?.channels.length}<button class="cstep" data-cchan aria-label="Open its channels" data-tip="Open its channels" onclick={(e) => (e.stopPropagation(), chanMenu(e, o))}>{@html ICON.vchart}</button>{/if}
                      </span>
                    </div>
                    {#if k.kind === 'check' && o && (o.verdict === 'cant' || o.verdict === 'err' || o.verdict === 'na') && o.message}<div class="cmsg muted">{o.message}</div>{/if}
                  {/if}
                {/each}
              {/if}
            {/each}
          {/if}
        </div>
      {/each}
      {#if ui.path && idle.length}
        <div class="cother">
          <div class="muted cotitle">Not running on this file</div>
          {#each idle as s (s.name)}
            <div class="coset" data-cother={s.name}><span class="csn">{s.name}</span><button class="linkbtn" data-cattach={s.name} onclick={() => attach(s.name)}>Run here</button><button class="cedit show" data-cedit={s.name} aria-label="Edit {s.name}" onclick={() => actions.checkEditor(s.name)}>{@html ICON.edit}</button></div>
          {/each}
        </div>
      {/if}
      {#if ui.path && applied.some((s) => !s.runs_on_any_file && (settings.checkAttach.files[ui.path] ?? []).includes(s.name))}
        <div class="cother">
          {#each applied.filter((s) => !s.runs_on_any_file && (settings.checkAttach.files[ui.path] ?? []).includes(s.name)) as s (s.name)}
            <div class="coset"><span class="muted">{s.name} runs on this file</span><button class="linkbtn" data-cdetach={s.name} onclick={() => detach(s.name)}>Stop</button></div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>
{#if menu}<Menu class="chmenu" label={menu?.label} anchor={menu?.anchor} at={menu?.at} items={menu?.items} onclose={() => (menu = null)} />{/if}

<style>
  .ckin {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 0 8px;
  }
  .cbadge {
    margin-left: 6px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--raised);
    font-size: 11px;
  }
  .cbadge.bad {
    background: color-mix(in srgb, var(--danger) 25%, transparent);
  }
  .ckbar {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 4px 12px;
  }
  .ckbar :global(svg) {
    width: 11px;
    height: 11px;
    margin-right: 4px;
  }
  .cknote {
    margin: 4px 12px;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .csethd {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-right: 8px;
  }
  .cshead,
  .cgrp {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px;
    border: 0;
    background: transparent;
    color: var(--fg);
    font: 12px var(--font-ui);
    text-align: left;
  }
  .cshead {
    flex: 1;
    min-width: 0;
  }
  .cgrp {
    width: 100%;
    padding-left: 22px;
  }
  .csn {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cgn {
    flex: none;
  }
  .cgs {
    margin-left: auto;
    font-size: 11px;
    white-space: nowrap;
  }
  .chv {
    display: inline-flex;
    width: 10px;
  }
  .chv.shut {
    transform: rotate(-90deg);
  }
  .cv {
    flex: none;
    width: 14px;
    text-align: center;
    font-size: 12px;
  }
  .csethd .cv {
    width: auto;
  }
  .cv.fail {
    color: var(--danger);
  }
  .cv.warn {
    color: var(--warn);
  }
  .cv.pass {
    color: var(--ok, #3fcf8e);
  }
  .cv.mark {
    color: var(--accent);
  }
  .cv.na,
  .cv.off,
  .cv.cant,
  .cv.err {
    color: var(--fg-muted);
  }
  .crow {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    margin: 0 6px 0 28px;
    padding: 0 6px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
  }
  .crow:hover,
  .crow.sel {
    background: var(--raised);
  }
  .crow.sel {
    outline: 1px solid var(--accent);
  }
  .crow.off {
    opacity: 0.55;
  }
  .ckn {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ckd {
    flex: none;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
  }
  .ckacts {
    display: flex;
    flex: none;
    opacity: 0;
  }
  .crow:hover .ckacts,
  .crow.sel .ckacts,
  .crow:focus-within .ckacts {
    opacity: 1;
  }
  .cstep {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 9px;
  }
  .cstep:hover {
    background: var(--border);
    color: var(--fg);
  }
  .cstep :global(svg) {
    width: 12px;
    height: 12px;
  }
  .cmsg {
    margin: 0 12px 2px 52px;
    font-size: 11px;
  }
  .cedit {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    opacity: 0;
  }
  .csethd:hover .cedit,
  .cedit:focus-visible,
  .cedit.show {
    opacity: 1;
  }
  .cedit :global(svg) {
    width: 12px;
    height: 12px;
  }
  .cother {
    margin: 6px 12px 0;
    font-size: 11.5px;
  }
  .cotitle {
    margin-bottom: 2px;
    font-size: 11px;
  }
  .coset {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 26px;
  }
  .coset .csn {
    flex: 1;
    font-weight: 400;
  }
</style>
