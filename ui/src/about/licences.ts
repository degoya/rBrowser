// The Licences tab's list (RB-010-18): the Rust crates and npm packages `scripts/licenses.sh`
// generates into `licenses/third-party.json`, and the bundled binaries of
// `licenses/bundled.json`, kept by hand. Filtered by name or licence, ignoring case.

import bundledFile from './licenses/bundled.json';
import thirdParty from './licenses/third-party.json';

export type ComponentKind = 'bundled' | 'rust' | 'npm';

/** `bundled`: in the package; `planned`: with a later milestone; `on-demand`: fetched when used. */
export type BundledStatus = 'bundled' | 'planned' | 'on-demand';

export interface Component {
  kind: ComponentKind;
  name: string;
  version: string;
  license: string;
  status?: BundledStatus;
}

interface Row {
  name: string;
  version: string;
  license: string;
}

interface BundledRow extends Row {
  id: string;
  status: BundledStatus;
}

export interface ThirdParty {
  rust: Row[];
  rustNotShipped: string[];
  npm: Row[];
  npmNotShipped: string[];
}

export const THIRD_PARTY: ThirdParty = thirdParty;
export const BUNDLED: BundledRow[] = bundledFile as BundledRow[];

/** Every component in the tab's order: the bundled binaries, the crates, the npm packages. */
export function allComponents(third: ThirdParty = THIRD_PARTY, bundled: BundledRow[] = BUNDLED): Component[] {
  return [
    ...bundled.map((row) => ({
      kind: 'bundled' as const,
      name: row.name,
      version: row.version,
      license: row.license,
      status: row.status,
    })),
    ...third.rust.map((row) => ({ kind: 'rust' as const, ...row })),
    ...third.npm.map((row) => ({ kind: 'npm' as const, ...row })),
  ];
}

/** The components whose name or licence contains `query`, ignoring case; all for a blank one. */
export function filterComponents(components: Component[], query: string): Component[] {
  const needle = query.trim().toLowerCase();
  if (needle === '') {
    return components;
  }
  return components.filter(
    (component) =>
      component.name.toLowerCase().includes(needle) || component.license.toLowerCase().includes(needle),
  );
}
