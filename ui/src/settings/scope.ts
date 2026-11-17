// Scopes at a field: the badge that says where the value comes from, and where a change is
// saved - the user settings, the project (`config.json`, checked in) or this machine only
// (`config.local.json`). A local-only key is locked out of the project, with its reason.

import type { Origin, Scope } from '../ipc/protocol';
import type { SettingDef } from './schema';

export interface Badge {
  labelKey: string;
  color: 'neutral' | 'primary' | 'info' | 'warning';
}

const BADGES: Record<Origin, Badge> = {
  default: { labelKey: 'settings.origin.default', color: 'neutral' },
  user: { labelKey: 'settings.origin.user', color: 'primary' },
  project: { labelKey: 'settings.origin.project', color: 'info' },
  local: { labelKey: 'settings.origin.local', color: 'warning' },
};

/** The badge of a value from `origin`. */
export function badgeOf(origin: Origin): Badge {
  return BADGES[origin];
}

export interface ScopeChoice {
  scope: Scope;
  labelKey: string;
  /** Why the scope cannot be chosen; absent when it can. */
  reasonKey?: string;
}

const SCOPE_LABELS: Record<Scope, string> = {
  user: 'settings.scope.user',
  project: 'settings.scope.project',
  local: 'settings.scope.local',
};

/**
 * The scopes a change of `def` may go to, in the order shown. A project key offers the project
 * and this machine; a local-only one shows the project locked with the reason; without an open
 * project both are locked.
 */
export function scopeChoices(def: SettingDef, hasProject: boolean): ScopeChoice[] {
  if (def.scopeAllowed.includes('user')) {
    return [{ scope: 'user', labelKey: SCOPE_LABELS.user }];
  }
  const noProject = hasProject ? {} : { reasonKey: 'settings.scope.no_project' };
  const project: ScopeChoice = def.localOnly
    ? { scope: 'project', labelKey: SCOPE_LABELS.project, reasonKey: 'settings.scope.local_only_reason' }
    : { scope: 'project', labelKey: SCOPE_LABELS.project, ...noProject };
  return [project, { scope: 'local', labelKey: SCOPE_LABELS.local, ...noProject }];
}

/** Where a change goes unless the person picks another scope: where the value is stored now,
 * else the first scope that can be chosen. */
export function defaultScope(def: SettingDef, origin: Origin, hasProject: boolean): Scope | null {
  const open = scopeChoices(def, hasProject).filter((choice) => choice.reasonKey === undefined);
  const stored = open.find((choice) => choice.scope === origin);
  return (stored ?? open[0])?.scope ?? null;
}
