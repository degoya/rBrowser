// The setup wizard's steps (RB-091-05, the owner's decision): welcome, plugins, project, basics,
// done - every one skippable - and, before the end, the recommended AI agent step (RB-091-09: it
// can use the project the step before set up). The hint "Set up project" opens the project step
// (`#/setup/project`), `#/setup/agent` the agent step.

import { routeHash } from '../settings/route';

export const STEPS = ['welcome', 'plugins', 'project', 'basics', 'agent', 'done'] as const;
export type StepId = (typeof STEPS)[number];

/** The step the route's key names (`project`, `agent`), else the welcome. */
export function startStep(routeKey: string | null): StepId {
  return routeKey === 'project' || routeKey === 'agent' ? routeKey : 'welcome';
}

/** A step's footer button (RB-091-08): its label names what the click does - "Install 3 plugins &
 * continue", "Write config & continue" - and `busy` holds it while the step works. */
export interface NextAction {
  key: string;
  args?: Record<string, string | number>;
  busy?: boolean;
}

/** What the wizard asks of the step it shows: the button, and the click that carries it out -
 * `true` when the wizard goes on. */
export interface StepControl {
  next: NextAction;
  proceed(): Promise<boolean>;
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
