import { readFileSync } from 'node:fs';

/** The release's version, from the workspace's Cargo.toml (`tools/check_versions.py` keeps every manifest on it), as a
 *  `define` the page shows in Settings: a page on a static host says which build it is. */
const toml = readFileSync(new URL('../../Cargo.toml', import.meta.url), 'utf8');
export const APP_VERSION = JSON.stringify(/^\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/m.exec(toml)?.[1] ?? 'dev');
