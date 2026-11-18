// The setup wizard's steps (RB-091-05, the owner's decision): welcome, plugins, project, basics,
// done - every one skippable. The hint "Set up project" opens the project step (`#/setup/project`).

import { routeHash } from '../settings/route';

export const STEPS = ['welcome', 'plugins', 'project', 'basics', 'done'] as const;
export type StepId = (typeof STEPS)[number];

/** The step the route's key names (`project`), else the welcome. */
export function startStep(routeKey: string | null): StepId {
  return routeKey === 'project' ? 'project' : 'welcome';
}

export function nextStep(step: StepId): StepId {
  return STEPS[Math.min(STEPS.indexOf(step) + 1, STEPS.length - 1)] ?? step;
}

export function previousStep(step: StepId): StepId {
  return STEPS[Math.max(STEPS.indexOf(step) - 1, 0)] ?? step;
}

/** The pages window's address of a setting or category (`ui.theme`, `plugins` with a tab). */
export function settingsHash(key: string, tab: string | null = null): string {
  return routeHash({ page: 'settings', key, tab });
}
