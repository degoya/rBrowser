// The picker's model, without DOM: the component's state, the groups the picker lists and the
// strength it shows. App.vue draws it; its texts come through the panel's `t` (panel-ui: catalogue
// messages, plurals included, a missing key logged once; RB-080-83).

import { isObject } from '../../../../web-common/src/is-object';

export const PREFIX = 'plugin.simulators.';

export interface Entry {
  id: string;
  group: string;
  /** The default strength (0 to 1); null for a simulation without one. */
  severity: number | null;
  native: boolean;
}

export interface ViewState {
  view: number;
  label: string;
  engine: string;
  type: string;
  severity: number | null;
  mode: string;
}

export interface State {
  active: number | null;
  views: ViewState[];
  catalog: Entry[];
}

export const GROUPS = ['color', 'vision', 'environment'] as const;
export const OFF: Entry = { id: 'none', group: 'none', severity: null, native: false };

/** A panel text: `key` below `plugin.simulators.` with its parameters filled in (panel-ui's `t`). */
export type Text = (key: string, params?: Record<string, unknown>) => string;

/** The name of a simulation. */
export const typeName = (t: Text, id: string): string => t(`types.${id}.name`);

type Json = Record<string, unknown>;
const strength = (value: unknown): number | null => (typeof value === 'number' && Number.isFinite(value) ? value : null);

function readView(value: unknown): ViewState | null {
  if (!isObject(value) || typeof value.view !== 'number') return null;
  if (typeof value.label !== 'string' || typeof value.type !== 'string' || typeof value.mode !== 'string') return null;
  return {
    view: value.view,
    label: value.label,
    engine: typeof value.engine === 'string' ? value.engine : '',
    type: value.type,
    severity: strength(value.severity),
    mode: value.mode,
  };
}

function readEntry(value: unknown): Entry | null {
  if (!isObject(value) || typeof value.id !== 'string' || typeof value.group !== 'string') return null;
  return { id: value.id, group: value.group, severity: strength(value.severity), native: value.native === true };
}

const list = <T>(value: unknown, read: (entry: unknown) => T | null): T[] =>
  Array.isArray(value) ? value.map(read).filter((entry): entry is T => entry !== null) : [];

/** The state in a `simulator_set` answer or a posted `state` event. */
export function readState(value: unknown): State {
  const json = isObject(value) ? value : {};
  return {
    active: typeof json.active === 'number' ? json.active : null,
    views: list(json.views, readView),
    catalog: list(json.catalog, readEntry),
  };
}

/** The picker's groups: Off, then the catalogue's three groups in order. */
export function groups(catalog: readonly Entry[]): Array<{ id: string; entries: Entry[] }> {
  return [
    { id: 'none', entries: [OFF] },
    ...GROUPS.map((id) => ({ id, entries: catalog.filter((entry) => entry.group === id) })),
  ];
}

/** One view in words: its label, its simulation and who draws it (never colour alone). */
export function viewRow(t: Text, view: ViewState): string {
  return t('panel.view_row', {
    label: view.label,
    type: typeName(t, view.type),
    mode: t(`panel.mode_${view.mode}`),
  });
}

/** The strength the slider shows for `type`: its strength in the active view, else its default;
 * null for a simulation without one. */
export function strengthFor(state: State, type: string): number | null {
  const own = state.views.find((view) => view.view === state.active);
  return own?.type === type ? own.severity : (state.catalog.find((entry) => entry.id === type)?.severity ?? null);
}

/** The `simulator_set` arguments: the active view (no target) or every view. */
export function request(type: string, severity: number | null, all: boolean): Record<string, unknown> {
  return {
    type,
    ...(severity === null ? {} : { severity }),
    ...(all ? { target: 'all' } : {}),
  };
}
