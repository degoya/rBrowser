// The palette's providers over the core's state (RB-060-11): the views (`@`), the project's
// breakpoints as "Set view to ...", the layout presets and the environments. Each list is also
// an argument step: "Set view to ..." asks for the breakpoint in a second step.

import type { ShellToCore, StateSnapshot, ViewId } from '../ipc/protocol';
import {
  STEP_ENVIRONMENT,
  STEP_FOCUS_VIEW,
  STEP_LAYOUT,
  STEP_SET_VIEW,
  type EntryScope,
  type PaletteEntry,
  type PaletteProvider,
  type ProviderContext,
} from './types';

/** The view a "Set view to ..." entry changes: the active one, else the first. */
export function targetView(state: StateSnapshot | null): ViewId | null {
  return state?.activeView ?? state?.views[0]?.id ?? null;
}

function stepEntry(step: string, titleKey: string, scope: EntryScope, t: ProviderContext['t']): PaletteEntry {
  return { key: `step:${step}`, scope, title: t(titleKey), group: t('palette.kind.step'), action: { kind: 'step', step } };
}

function send(message: ShellToCore): PaletteEntry['action'] {
  return { kind: 'send', message };
}

const viewsProvider: PaletteProvider = {
  id: 'views',
  entries: ({ state, t }) => {
    const views = state?.views ?? [];
    if (views.length === 0) {
      return [];
    }
    return [
      stepEntry(STEP_FOCUS_VIEW, 'palette.step.focus_view', 'view', t),
      ...views.map(
        (view): PaletteEntry => ({
          key: `view:${view.id}`,
          scope: 'view',
          parent: STEP_FOCUS_VIEW,
          title: t('palette.entry.focus_view', { label: view.label, width: view.width, height: view.height }),
          group: t('palette.kind.view'),
          description: view.url,
          keywords: [view.breakpoint ?? '', view.engine].filter((k) => k !== ''),
          action: send({ type: 'setActiveView', view: view.id }),
        }),
      ),
    ];
  },
};

const breakpointsProvider: PaletteProvider = {
  id: 'breakpoints',
  entries: ({ state, t }) => {
    const view = targetView(state);
    if (state === null || view === null) {
      return [];
    }
    return [
      stepEntry(STEP_SET_VIEW, 'palette.step.set_view', 'other', t),
      ...state.breakpoints.map(
        (breakpoint): PaletteEntry => ({
          key: `set-view:${breakpoint.id}`,
          scope: 'other',
          parent: STEP_SET_VIEW,
          title: t('palette.entry.set_view', {
            label: breakpoint.label,
            width: breakpoint.width,
            height: breakpoint.height,
          }),
          group: t('palette.kind.breakpoint'),
          keywords: [breakpoint.id],
          action: send({ type: 'setBreakpoint', view, breakpoint: breakpoint.id }),
        }),
      ),
    ];
  },
};

const layoutsProvider: PaletteProvider = {
  id: 'layouts',
  entries: ({ state, t }) => {
    const presets = state?.layout.presets ?? [];
    if (presets.length === 0) {
      return [];
    }
    return [
      stepEntry(STEP_LAYOUT, 'palette.step.layout', 'other', t),
      ...presets.map(
        (id): PaletteEntry => ({
          key: `layout:${id}`,
          scope: 'other',
          parent: STEP_LAYOUT,
          title: t('palette.entry.layout', { id }),
          group: t('palette.kind.layout'),
          action: send({ type: 'selectLayoutPreset', id }),
        }),
      ),
    ];
  },
};

const environmentsProvider: PaletteProvider = {
  id: 'environments',
  entries: ({ state, t }) => {
    const environments = state?.environments ?? [];
    if (environments.length === 0) {
      return [];
    }
    return [
      stepEntry(STEP_ENVIRONMENT, 'palette.step.environment', 'other', t),
      ...environments.map(
        (environment): PaletteEntry => ({
          key: `environment:${environment.id}`,
          scope: 'other',
          parent: STEP_ENVIRONMENT,
          title: t('palette.entry.environment', { id: environment.id }),
          group: t('palette.kind.environment'),
          description: environment.url,
          action: send({ type: 'selectEnvironment', id: environment.id }),
        }),
      ),
    ];
  },
};

export const stateProviders: readonly PaletteProvider[] = [
  viewsProvider,
  breakpointsProvider,
  layoutsProvider,
  environmentsProvider,
];
