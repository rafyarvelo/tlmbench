// Folders in browser-only mode (release plan R2.2, ledger D-brow-10): `showDirectoryPicker` where the browser has it, a
// read-only listing for Explorer as the app's `list_folder` gives, and the files inside opened by their handles. Nothing is
// ever written to the folder (the picker is asked for read access only). The handle is kept in the browser's IndexedDB so a
// reload can list the folder again, which needs the browser's permission again unless it remembers it.
//
// Paths: the folder is `local-folder://name`, a folder inside it `local-folder://name/sub`, a file `local://name/sub/f.csv`
// (the same prefix the page's picked files use, so a file opened from a folder comes back after a reload like any other).

import type { FolderEntry } from './ui.svelte';

export const FOLDER_PREFIX = 'local-folder://';
const FILE_PREFIX = 'local://';
const DB = 'tlmbench-folders';

/** The pieces of the File System Access API the DOM types of this project don't have. */
type DirHandle = FileSystemDirectoryHandle & {
  queryPermission?(d: { mode: 'read' }): Promise<PermissionState>;
  requestPermission?(d: { mode: 'read' }): Promise<PermissionState>;
};

/** Whether this browser can open a folder (Chromium; not Firefox or Safari today). */
export const canPickFolder = (): boolean => typeof (globalThis as { showDirectoryPicker?: unknown }).showDirectoryPicker === 'function';

const roots = new Map<string, DirHandle>();
const fileHandles = new Map<string, FileSystemFileHandle>();

function db(): Promise<IDBDatabase> {
  return new Promise((ok, fail) => {
    const r = indexedDB.open(DB, 1);
    r.onupgradeneeded = () => r.result.createObjectStore('handles');
    r.onsuccess = () => ok(r.result);
    r.onerror = () => fail(r.error);
  });
}

async function keep(name: string, h: DirHandle): Promise<void> {
  try {
    const d = await db();
    await new Promise<void>((ok, fail) => {
      const t = d.transaction('handles', 'readwrite');
      t.objectStore('handles').put(h, name);
      t.oncomplete = () => ok();
      t.onerror = () => fail(t.error);
    });
  } catch {
    /* no storage: the folder is chosen again after a reload */
  }
}

async function recall(name: string): Promise<DirHandle | null> {
  try {
    const d = await db();
    return await new Promise<DirHandle | null>((ok, fail) => {
      const q = d.transaction('handles').objectStore('handles').get(name);
      q.onsuccess = () => ok((q.result as DirHandle | undefined) ?? null);
      q.onerror = () => fail(q.error);
    });
  } catch {
    return null;
  }
}

/** The picker: the folder chosen as its path, or null when the person cancels. Read access only. */
export async function pickFolder(): Promise<string | null> {
  try {
    const h = (await (globalThis as unknown as { showDirectoryPicker(o: { mode: 'read' }): Promise<DirHandle> }).showDirectoryPicker({ mode: 'read' }));
    roots.set(h.name, h);
    void keep(h.name, h);
    return `${FOLDER_PREFIX}${h.name}`;
  } catch {
    return null; // cancelled
  }
}

const OPENABLE = /\.(csv|tsv|txt|tlm|parquet)$/i;

/** What the page knows about files it has no reader of its own for: a descriptor that names the file, and whether its
 *  first bytes are binary (then the descriptor form opens for it, as in the app). */
export interface Readers {
  claims(name: string): boolean;
  binary(head: Uint8Array): boolean;
}

async function rootOf(name: string): Promise<DirHandle> {
  let root = roots.get(name) ?? null;
  if (!root) {
    root = await recall(name);
    if (root) roots.set(name, root);
  }
  if (!root) throw new Error('Choose the folder again: a browser tab doesn’t keep folders from an earlier visit.');
  return root;
}

/** The folder's tree for Explorer, as `list_folder` gives it: dot files and folders left out, folders before files, a folder
 *  with nothing in it left out, every file marked openable or not (a binary file no descriptor names: `needsDescriptor`). */
export async function listFolder(path: string, readers?: Readers): Promise<FolderEntry> {
  const name = path.slice(FOLDER_PREFIX.length).split('/')[0];
  const root = await rootOf(name);
  if ((await root.queryPermission?.({ mode: 'read' })) !== 'granted') {
    const asked = await root.requestPermission?.({ mode: 'read' }).catch(() => 'denied' as PermissionState);
    if (asked !== 'granted') throw new Error('Choose the folder again: the browser asks for permission on each visit.');
  }
  fileHandles.clear();
  async function walk(h: DirHandle, rel: string, depth: number): Promise<FolderEntry | null> {
    const children: FolderEntry[] = [];
    if (depth < 8) {
      for await (const e of h.values()) {
        if (e.name.startsWith('.')) continue;
        const sub = rel ? `${rel}/${e.name}` : e.name;
        if (e.kind === 'directory') {
          const d = await walk(e as DirHandle, sub, depth + 1);
          if (d) children.push(d);
        } else {
          const fp = `${FILE_PREFIX}${name}/${sub}`;
          fileHandles.set(fp, e as FileSystemFileHandle);
          const f = await (e as FileSystemFileHandle).getFile();
          const own = OPENABLE.test(e.name);
          const claimed = !own && !!readers?.claims(e.name);
          const needs = !own && !claimed && !!readers && f.size > 0 && readers.binary(new Uint8Array(await f.slice(0, 4096).arrayBuffer()));
          children.push({ path: fp, name: e.name, dir: false, bytes: f.size, openable: own || claimed || needs, ...(needs ? { needsDescriptor: true } : {}), children: [] });
        }
      }
    }
    children.sort((a, b) => Number(b.dir) - Number(a.dir) || a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    if (depth > 0 && !children.length) return null;
    return { path: rel ? `${FOLDER_PREFIX}${name}/${rel}` : `${FOLDER_PREFIX}${name}`, name: h.name, dir: true, bytes: 0, children };
  }
  return (await walk(root, '', 0)) as FolderEntry;
}

/** A file listed from a folder, by its path (null before the folder is listed, or for a path that is not from one). */
export async function folderFile(path: string): Promise<File | null> {
  const h = fileHandles.get(path);
  return h ? h.getFile() : null;
}

/** The folder's own `.tlmbench/` as texts by their path inside it (`widgets/dial/widget.toml`), for the core to read as
 *  the app reads one; none when it has none. Text files only, and none over 1 MB. Read access only, as everything here. */
export async function readConfig(path: string): Promise<Record<string, string> | null> {
  const name = path.slice(FOLDER_PREFIX.length).split('/')[0];
  let dir: DirHandle;
  try {
    dir = (await (await rootOf(name)).getDirectoryHandle('.tlmbench')) as DirHandle;
  } catch {
    return null;
  }
  const out: Record<string, string> = {};
  async function walk(h: DirHandle, rel: string, depth: number): Promise<void> {
    if (depth > 3) return;
    for await (const e of h.values()) {
      const sub = rel ? `${rel}/${e.name}` : e.name;
      if (e.kind === 'directory') await walk(e as DirHandle, sub, depth + 1);
      else if (/\.(toml|json|svg|html|js|css|txt)$/i.test(e.name)) {
        const f = await (e as FileSystemFileHandle).getFile();
        if (f.size <= 1 << 20) out[sub] = await f.text();
      }
    }
  }
  await walk(dir, '', 0);
  return out;
}
