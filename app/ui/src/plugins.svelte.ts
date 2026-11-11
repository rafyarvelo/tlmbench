// Plugins as the core last read them: Settings lists them, and the Alignment section offers the estimators.
import { invoke } from '@tauri-apps/api/core';

export interface PluginRow {
  file: string;
  path: string;
  tier: 'wasm' | 'native';
  kind: 'decoder' | 'estimator' | null;
  name: string;
  version: string | null;
  description: string | null;
  /** Found in the open folder's .tlmbench/plugins/ and not in your own folder. */
  shared: boolean;
  /** For a shared plugin: its plugins folder, which trusting is keyed by. */
  folder: string | null;
  status: { state: 'loaded' | 'off' | 'untrusted' | 'failed'; why?: string };
}

export const pluginsState = $state<{ list: PluginRow[] }>({ list: [] });

/** The list as of the core's last scan. */
export async function readPlugins(): Promise<void> {
  pluginsState.list = await invoke<PluginRow[]>('plugins_list').catch(() => pluginsState.list);
}

/** Reads the plugins folders again; `nativeOn` is which native plugins are turned on, `trusted` which folders' shared plugins may load. */
export async function scanPlugins(nativeOn?: string[], trusted?: string[]): Promise<void> {
  pluginsState.list = await invoke<PluginRow[]>('plugins_scan', { nativeOn: nativeOn ?? null, trusted: trusted ?? null }).catch(() => pluginsState.list);
}

/** The alignment estimators that are loaded. */
export const estimators = (): PluginRow[] => pluginsState.list.filter((p) => p.kind === 'estimator' && p.status.state === 'loaded');
