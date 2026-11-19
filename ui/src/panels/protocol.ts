// The plugin panels' messages (RB-060-12, docs/plugins.md "Page scripts and panels"): between the
// panels window and the core (`pluginPanel`, read by `crates/rbrowser/src/plugins/panels.rs`), and
// between the panels window and a panel's sandboxed frame (`postMessage`, `bridge.ts`).

import type { PluginGrantDetails } from '../ipc/trustGrants';

/** Every panel shows in the window's content area; a `dialog` panel is only never the one the
 * window opens unasked (RB-080-59). */
export type PanelPlacement = 'sidebar' | 'dialog';

/** One panel the core offers. */
export interface PanelInfo {
  plugin: string;
  panel: string;
  /** The title in the plugin's locales. */
  title: string;
  placement: PanelPlacement;
  /** The manifest's Lucide icon name (RB-080-61), checked by the plugin host; drawn only when the
   * shell bundles it (`icons.ts`, `panelIcon`). */
  icon?: string;
  /** `rbrowser://plugin/<id>/panel/...`, the frame's source. */
  url: string;
}

/** A request of the panels window to the core. */
export type PanelRequest =
  | { op: 'list' }
  | { op: 'call'; plugin: string; command?: string; tool?: string; args?: unknown }
  | { op: 'settings'; plugin: string }
  /** The installed plugins, for Settings > Plugins (RB-080-51); a read. */
  | { op: 'installed' }
  /** The panel the page shows now, so the panels move with it (RB-080-57); opens nothing. */
  | { op: 'selected'; plugin: string; panel: string }
  /** The same for a core panel of the page (`findings`, RB-080-69). */
  | { op: 'selectedCore'; panel: string }
  /** The plugin catalogue of Settings > Plugins > Catalog (RB-091-01), its repositories fetched
   * again first with `refresh`. */
  | { op: 'catalog'; refresh?: boolean }
  /** Downloads and reviews a catalogue offer: its full grants and its key; installs nothing. */
  | { op: 'catalogPrepare'; repository: string; id: string; version: string }
  /** Installs a reviewed catalogue offer with what the dialog showed (RB-091-01). */
  | {
      op: 'catalogInstall';
      repository: string;
      id: string;
      version: string;
      consent: { capabilities: string[]; grants: PluginGrantDetails; keyFingerprint: string | null };
    }
  /** Removes a plugin the catalogue installed (RB-091-01). */
  | { op: 'catalogRemove'; id: string }
  /** The first-party plugins the first-run wizard offers (RB-091-04, RB-091-05); a read. */
  | { op: 'available' }
  /** Installs the chosen first-party plugins, each from the repository first and the bundle
   * otherwise, with `pluginInstallProgress` messages (RB-091-04). */
  | { op: 'installChosen'; ids: string[] }
  /** Takes the named plugins out of the choice and removes the catalogue's copies (RB-091-04). */
  | { op: 'removeChosen'; ids: string[] }
  /** The first-run wizard's Skip: the recommended set is chosen, the choice no longer pending. */
  | { op: 'applyRecommended' };

/** A message for a plugin's panels: from its page scripts (with their view) or its component. */
export interface PanelEvent {
  plugin: string;
  view?: number;
  message: unknown;
}

/** The panel a command asked for (its answer's `$open`), to select in an open window; or a core
 * panel of the page (`panels.findings`, RB-080-69). */
export type PanelSelect = { plugin: string; panel: string } | { core: string };

/** The answer to `installed`: the rows of `crates/rbrowser/src/plugins/list.rs`, read by
 * `settings/plugins/installed.ts`; `loading` while the core still loads its plugins. */
export interface InstalledMessage {
  loading: boolean;
  plugins: unknown[];
  /** Per source the folder the start read, and the one it did not find. */
  folders?: Record<string, string>;
  missing?: Record<string, string>;
}

/** The core's answer to a `PanelRequest`, or an event. */
export type PanelMessage =
  | { panels: PanelInfo[] }
  | InstalledMessage
  | { value: unknown }
  | { strings: Record<string, string>; settings: Record<string, unknown> }
  | { event: PanelEvent }
  | { select: PanelSelect }
  /** The panels offered changed - the plugins are loaded, a panel hidden, a project plugin under
   * new trust: a panels page asks for the list again (RB-080-97, `plugins/panel_list.rs`). */
  | { panelsChanged: true }
  /** One chosen plugin's install began or ended (RB-091-04, `plugins/bundle_install.rs`). */
  | {
      pluginInstallProgress: {
        id: string;
        done: number;
        total: number;
        state: 'installing' | 'installed' | 'failed';
        error?: string;
      };
    };
