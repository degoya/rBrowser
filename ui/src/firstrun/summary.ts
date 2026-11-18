// The setup wizard's last step (RB-091-05): what was done, as sentences, and where each thing
// lives in the settings afterwards.

import type { SetupAnswer, SetupLayout } from '../ipc/protocol';
import type { ThemeSetting } from '../theme/theme';
import type { PluginsDone } from './plugins';

export interface BasicsChoice {
  theme: ThemeSetting;
  /** `null`: no project to write it into. */
  layout: SetupLayout | null;
  sync: boolean | null;
}

export interface SetupSummary {
  plugins: PluginsDone | null;
  project: Extract<SetupAnswer, { op: 'written' }> | null;
  basics: BasicsChoice | null;
}

export interface Line {
  key: string;
  args?: Record<string, string | number>;
}

const THEME_LINES: Record<ThemeSetting, string> = {
  system: 'firstrun.done.theme.system',
  light: 'firstrun.done.theme.light',
  dark: 'firstrun.done.theme.dark',
};

const LAYOUT_LINES: Record<SetupLayout, string> = {
  grid: 'firstrun.done.layout.grid',
  canvas: 'firstrun.done.layout.canvas',
};

export const EMPTY_SUMMARY: SetupSummary = { plugins: null, project: null, basics: null };

/** One sentence per step that did something; one that says nothing was changed otherwise. */
export function summaryLines(summary: SetupSummary): Line[] {
  const lines: Line[] = [];
  if (summary.plugins !== null) {
    const installed = summary.plugins.results.filter((result) => result.ok).length;
    const failed = summary.plugins.results.length - installed;
    lines.push({ key: 'firstrun.done.plugins', args: { count: installed } });
    if (failed > 0) lines.push({ key: 'firstrun.done.plugins_failed', args: { count: failed } });
    if (summary.plugins.removed.length > 0) {
      lines.push({ key: 'firstrun.done.plugins_removed', args: { count: summary.plugins.removed.length } });
    }
  }
  if (summary.project !== null) {
    lines.push({ key: 'firstrun.done.project', args: { path: summary.project.path } });
  }
  if (summary.basics !== null) {
    lines.push({ key: THEME_LINES[summary.basics.theme] });
    if (summary.basics.layout !== null) lines.push({ key: LAYOUT_LINES[summary.basics.layout] });
    if (summary.basics.sync !== null) lines.push({ key: summary.basics.sync ? 'firstrun.done.sync_on' : 'firstrun.done.sync_off' });
  }
  return lines.length > 0 ? lines : [{ key: 'firstrun.done.nothing' }];
}

/** Where each thing lives in the settings: the label's key, the setting or category, its tab. */
export const SETTINGS_PLACES: readonly { labelKey: string; key: string; tab: string | null }[] = [
  { labelKey: 'firstrun.done.where.plugins', key: 'plugins', tab: 'catalog' },
  { labelKey: 'firstrun.done.where.project', key: 'startUrl', tab: null },
  { labelKey: 'firstrun.done.where.theme', key: 'ui.theme', tab: null },
  { labelKey: 'firstrun.done.where.wizard', key: 'ui.firstRunWizard', tab: null },
];
