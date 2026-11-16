// What a panel says above its DevTools (RB-080-24): whether the followed view runs the
// framework, whether rBrowser's hook reached its document, and the hints that need a reload
// or a build flag. The keys are the plugin's locale keys under `panel.`.

import type { Framework, FrameworkReport } from './protocol';

/** The panel's status: a locale key under `panel.`. */
export type Status = 'noView' | 'waiting' | 'noHook' | 'late' | 'vueProd' | 'connected';

/** The status of `framework` in `view` after the probe's last `report` of it (null: none yet). */
export function statusOf(framework: Framework, view: number | null, report: FrameworkReport | null): Status {
  if (view === null) return 'noView';
  const info = report?.[framework];
  if (!info) return 'waiting';
  if (!info.hook) return 'noHook';
  if (!info.devtools) return framework === 'vue' ? 'vueProd' : 'late';
  return 'connected';
}

/** The framework's display name. */
export function nameOf(framework: Framework): string {
  return framework === 'vue' ? 'Vue' : 'React';
}
