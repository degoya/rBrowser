// The side column's tabs (RB-096-11) and how a link shows one (RB-094-27): a link to the private
// values - a text that names API client › Variables - shows the Variables tab here; one from
// elsewhere (the status bar, the network panel, a status line) reaches the core as a link to
// `api-client.secrets`, which opens this panel and has its next `state` carry `tab`.
import type { InjectionKey } from 'vue';

/** The tabs, each named by its label and tooltip. */
export const SIDE_TABS = [
  { name: 'requests', icon: 'i-lucide-list-tree' },
  { name: 'variables', icon: 'i-lucide-variable' },
  { name: 'import', icon: 'i-lucide-file-input' },
  { name: 'export', icon: 'i-lucide-file-output' },
] as const;

export type SideTab = (typeof SIDE_TABS)[number]['name'];

/** Whether `name` is one of the tabs: a state's `tab` names nothing else. */
export function isSideTab(name: unknown): name is SideTab {
  return SIDE_TABS.some((tab) => tab.name === name);
}

/** Shows a side tab; the panel provides it, a link inside the panel takes it. */
export const SHOW_SIDE_TAB: InjectionKey<(tab: SideTab) => void> = Symbol('showSideTab');
