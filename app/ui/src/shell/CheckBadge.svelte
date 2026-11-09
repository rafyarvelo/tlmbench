<!-- ✓ / ! / ✗ and a count: what the checks that ran on a file found. Hover lists the groups that failed; a click opens
     the Rules & checks panel. -->
<script lang="ts">
  import { summary } from '../checks.svelte';
  import { setSetting } from '../settings.svelte';

  let { path, open = true }: { path: string; open?: boolean } = $props();
  const s = $derived(open || path ? summary(path) : null);
  const text = $derived(s ? (s.failed ? `✗ ${s.failed}` : s.warned ? `! ${s.warned}` : `✓ ${s.passed}`) : '');
  const tip = $derived(s ? (s.groups.length ? `Failed or warned: ${s.groups.join(', ')}` : 'Every check passed') : '');
</script>

{#if s}
  <button class="cbadge" class:bad={s.failed > 0} class:warn={!s.failed && s.warned > 0} data-cn={text} data-chkfile={path} data-tip={tip} aria-label="Checks: {text}" onclick={(e) => (e.stopPropagation(), setSetting('leftPanel', 'checks'))}>{text}</button>
{/if}

<style>
  .cbadge {
    flex: none;
    margin-left: 6px;
    padding: 0 6px;
    border: 0;
    border-radius: 999px;
    background: color-mix(in srgb, var(--ok, #3fcf8e) 22%, transparent);
    color: var(--fg);
    font: 11px var(--font-data);
  }
  .cbadge.bad {
    background: color-mix(in srgb, var(--danger) 28%, transparent);
  }
  .cbadge.warn {
    background: color-mix(in srgb, var(--warn) 28%, transparent);
  }
</style>
