// A stand-in core for `npm run dev` outside CEF and for the tests: it keeps a state, answers
// like the core and pushes `state` events after every change.

import { sampleState } from './mockState';
import type { Activity, ActivityBarState } from './activity';
import {
  IpcErrorCode,
  type CoreToShell,
  type JsonValue,
  type ShellToCore,
  type StateSnapshot,
  type StatusMessage,
} from './protocol';
import { IpcError, type Transport } from './transport';

export type MockTheme = 'system' | 'light' | 'dark';

export interface MockOptions {
  state?: StateSnapshot;
  theme?: MockTheme;
  /** Delay of every answer, to see loading states in dev. */
  latencyMs?: number;
  /** The status bar's history the core answers with (RB-080-15). */
  statusMessages?: StatusMessage[];
  /** Further setting values `getSettings` answers, by key. */
  settings?: Record<string, unknown>;
  /** The text size `getTextScale` answers, in percent (RB-090-26); 100 by default. */
  textScale?: number;
}

export interface MockTransport extends Transport {
  readonly state: StateSnapshot;
  /** Every request, in order - but the status bar's reports, which go to `reports`. */
  readonly sent: ShellToCore[];
  /** The messages the shell reported for the status bar (`reportStatus`), in order. */
  readonly reports: Extract<ShellToCore, { type: 'reportStatus' }>[];
  /** Pushes an event as the core would. */
  emit(message: CoreToShell): void;
}

const ACK: CoreToShell = { type: 'ack' };

export function createMockTransport(options: MockOptions = {}): MockTransport {
  const state = options.state ?? sampleState();
  const theme = options.theme ?? 'system';
  const listeners = new Set<(message: CoreToShell) => void>();
  const sent: ShellToCore[] = [];
  const reports: Extract<ShellToCore, { type: 'reportStatus' }>[] = [];
  const history = [state.url];
  let position = 0;
  // The status bar's log: a report goes in and out as an event, as in the core (RB-080-15).
  const statusMessages = [...(options.statusMessages ?? [])];
  const report = (message: Extract<ShellToCore, { type: 'reportStatus' }>): CoreToShell => {
    const { type: _type, ...notification } = message;
    const entry: StatusMessage = {
      ...notification,
      id: (statusMessages.at(-1)?.id ?? 0) + 1,
      at: Date.now(),
    };
    statusMessages.push(entry);
    emit({ type: 'notification', ...notification });
    emit({ type: 'statusMessage', ...entry });
    return ACK;
  };

  const emit = (message: CoreToShell): void => {
    listeners.forEach((listener) => listener(message));
  };
  const pushState = (): CoreToShell => {
    emit({ type: 'state', ...structuredClone(state) });
    return ACK;
  };
  const goTo = (url: string): void => {
    state.url = url;
    state.views.forEach((view) => {
      view.url = url;
    });
  };

  const handle = (message: ShellToCore): CoreToShell => {
    switch (message.type) {
      case 'getState':
        return { type: 'state', ...structuredClone(state) };
      case 'navigate':
        history.splice(position + 1, history.length, message.url);
        position = history.length - 1;
        goTo(message.url);
        return pushState();
      case 'back':
      case 'forward': {
        const next = position + (message.type === 'back' ? -1 : 1);
        const url = history[next];
        if (url !== undefined) {
          position = next;
          goTo(url);
        }
        return pushState();
      }
      case 'reload':
        return pushState();
      case 'setSync':
        if (message.feature === 'all') {
          state.sync.enabled = message.enabled;
        } else {
          state.sync.features[message.feature] = message.enabled;
        }
        return pushState();
      case 'selectLayoutPreset':
        if (!state.layout.presets.includes(message.id)) {
          throw new IpcError(IpcErrorCode.NotFound, `no layout preset ${message.id}`);
        }
        state.layout.preset = message.id;
        return pushState();
      case 'selectEnvironment': {
        const environment = state.environments.find((e) => e.id === message.id);
        if (environment === undefined) {
          throw new IpcError(IpcErrorCode.NotFound, `no environment ${message.id}`);
        }
        state.environment = environment.id;
        goTo(environment.url);
        return pushState();
      }
      case 'setActiveView':
        state.activeView = message.view;
        return pushState();
      case 'openSettings':
      case 'dismissPalette':
      case 'dismissHistory':
      // The canvas board's drags (RB-080-04): the mock keeps no canvas.
      case 'canvasDrag':
      case 'canvasPan':
      case 'canvasStep':
      case 'canvasSizePick':
      // The native menus (RB-080-46): the mock shows none.
      case 'canvasMenu':
      case 'addViewMenu':
      case 'canvasZoomMenu':
      case 'nativeMenu': // the toolbar's pulldowns (RB-090-41)
      case 'canvasZoom':
      case 'canvasPainted': // the board's answer (RB-090-55)
      // The extension prompt's answer (RB-080-23): the mock installs nothing.
      case 'extensionDecision':
      // The toolbar's rows (RB-080-52): the mock has no window to make room in.
      case 'toolbarRows':
        return ACK;
      // The activity bar (RB-090-67): the place granted and the folded entries, as the core keeps them.
      case 'activityBar':
        state.activityBar = { ...state.activityBar, place: message.place };
        return pushState();
      case 'collapseActivity':
        state.activityBar = collapseActivity(state, message.activity, message.collapsed);
        return pushState();
      case 'getStatusMessages':
        return { type: 'statusMessages', items: structuredClone(statusMessages) };
      case 'getTextScale':
        return { type: 'textScale', percent: options.textScale ?? 100 };
      case 'reportStatus':
        return report(message);
      case 'getSettings':
        return {
          type: 'settings',
          values: [
            { key: 'ui.theme', value: theme, origin: 'user' },
            ...Object.entries(options.settings ?? {}).map(([key, value]) => ({
              key,
              value: value as JsonValue,
              origin: 'user' as const,
            })),
          ],
        };
      default:
        throw new IpcError(IpcErrorCode.NotImplemented, `the mock core does not do ${message.type}`);
    }
  };

  return {
    state,
    sent,
    reports,
    emit,
    async send(message) {
      if (message.type === 'reportStatus') {
        reports.push(message);
      } else {
        sent.push(message);
      }
      if (options.latencyMs !== undefined && options.latencyMs > 0) {
        await new Promise((resolve) => setTimeout(resolve, options.latencyMs));
      }
      return handle(message);
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

/** `collapseActivity` as the core does it (`crate::activity_bar::collapse`). */
function collapseActivity(state: StateSnapshot, activity: Activity, collapsed: boolean): ActivityBarState {
  const bar = { ...state.activityBar };
  if (activity === 'recorder' && collapsed && state.recorder === undefined) {
    delete bar.recorded;
    return bar;
  }
  const others = (bar.collapsed ?? []).filter((entry) => entry !== activity);
  bar.collapsed = collapsed ? [...others, activity] : others;
  return bar;
}
