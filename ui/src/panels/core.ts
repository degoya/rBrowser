// The core's own panels in the panels page (RB-080-69): Findings had a window of its own; it is
// the first entry of the side bar now and shows in the content area as a component of the page -
// no plugin frame, the page's own rights (`crates/rbrowser/src/panel_dock/core_panels.rs`). A core
// panel's key has no dot, so it never meets a plugin's `<plugin>.<panel>`.

import { DOCK_ICONS, panelIcon } from './icons';
import type { PanelInfo } from './protocol';

/** One entry of the side bar: its key, its title and its icon class. */
export interface SideEntry {
  key: string;
  title: string;
  icon: string;
}

export const FINDINGS = 'findings';

/** The core panels, in the side bar's order, above the plugins' panels. */
export const CORE_PANELS: readonly { key: string; titleKey: string; icon: string }[] = [
  { key: FINDINGS, titleKey: 'findings.title', icon: DOCK_ICONS.findings },
];

export function isCorePanel(key: string | null): boolean {
  return CORE_PANELS.some((panel) => panel.key === key);
}

/** A plugin panel's key, `<plugin>.<panel>`. */
export const keyOf = (panel: PanelInfo): string => `${panel.plugin}.${panel.panel}`;

/** The side bar's entries: the core panels (titled by `t`), then the plugins' panels. */
export function sideEntries(panels: PanelInfo[], t: (key: string) => string): SideEntry[] {
  return [
    ...CORE_PANELS.map((panel) => ({ key: panel.key, title: t(panel.titleKey), icon: panel.icon })),
    ...panels.map((panel) => ({ key: keyOf(panel), title: panel.title, icon: panelIcon(panel.icon) })),
  ];
}
