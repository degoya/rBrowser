// The React DevTools panel (RB-080-24), built into ../panel/dist/react.js: the React DevTools
// frontend of `react-devtools-inline` (components, props, hooks, the profiler), its bridge over
// the active view's channel instead of `postMessage`. The frontend is initialised before the
// backend is told to connect, as the package requires. The panel follows the active view: it
// asks the component every `FOLLOW_MS`, disconnects the old view's backend and connects the new
// one's with a fresh bridge and store.

import { createBridge, createStore, initialize, type FrontendBridge, type WallMessage } from 'react-devtools-inline/frontend';
import { createElement, type ComponentType } from 'react';
import { createRoot } from 'react-dom/client';
import { createPanelBridge, type PanelBridge } from './panel-bridge';
import type { FrameworkReport } from './protocol';
import { nameOf, statusOf } from './status';
import { openViewChannel, watchProbe, type ViewChannel } from './view-channel';

const PREFIX = 'plugin.framework-devtools.';
/** How often the panel asks which view is active, in ms. */
const FOLLOW_MS = 1500;

interface Session {
  view: number;
  channel: ViewChannel;
  bridge: FrontendBridge;
  DevTools: ComponentType<{ browserTheme?: 'light' | 'dark'; showTabBar?: boolean }>;
}

const $ = (id: string): HTMLElement => {
  const element = document.getElementById(id);
  if (!element) throw new Error(`no element #${id}`);
  return element;
};

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function theme(): 'light' | 'dark' {
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

class ReactPanel {
  private readonly panel: PanelBridge;
  private readonly root = createRoot($('root'));
  private session: Session | null = null;
  private readonly reports = new Map<number, FrameworkReport>();

  constructor(panel: PanelBridge) {
    this.panel = panel;
    panel.onSettings(() => this.renderTexts());
    watchProbe(panel, (view, report) => {
      this.reports.set(view, report);
      this.renderStatus();
    });
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => this.render());
  }

  start(): void {
    this.renderTexts();
    this.panel.requestSettings();
    const follow = async (): Promise<void> => {
      try {
        const answer = await this.panel.call('active_view');
        const view = isObject(answer) && typeof answer.view === 'number' ? answer.view : null;
        await this.follow(view);
      } catch {
        // The host did not answer: the next tick asks again.
      }
    };
    void follow();
    setInterval(() => void follow(), FOLLOW_MS);
  }

  private async follow(view: number | null): Promise<void> {
    if (this.session?.view === view) return;
    if (this.session) {
      const { channel, bridge } = this.session;
      this.session = null;
      this.render();
      bridge.shutdown();
      await channel.post({ t: 'disconnect' }).catch(() => undefined);
      channel.close();
    }
    if (view !== null) {
      const channel = openViewChannel(this.panel, view, 'react');
      const bridge = createBridge(window, {
        listen(fn: (message: WallMessage) => void) {
          return channel.on((envelope) => {
            if (envelope.t !== 'm' || !isObject(envelope.d)) return;
            fn({ event: String(envelope.d.event), payload: envelope.d.payload });
          });
        },
        send(event: string, payload: unknown) {
          void channel.post({ t: 'm', d: { event, payload: payload ?? null } }).catch(() => undefined);
        },
      });
      const store = createStore(bridge);
      const DevTools = initialize(window, { bridge, store });
      this.session = { view, channel, bridge, DevTools };
      this.render();
      // The frontend listens now: the backend may start.
      await channel.post({ t: 'connect' }).catch(() => undefined);
    }
    this.renderStatus();
  }

  private render(): void {
    const session = this.session;
    if (!session) {
      this.root.render(null);
      return;
    }
    this.root.render(
      createElement(session.DevTools, { key: session.view, browserTheme: theme(), showTabBar: true }),
    );
  }

  private renderTexts(): void {
    document.title = this.panel.text('panels.react');
    this.renderStatus();
  }

  private renderStatus(): void {
    const view = this.session?.view ?? null;
    const report = view === null ? null : (this.reports.get(view) ?? null);
    const status = statusOf('react', view, report);
    const params = { framework: nameOf('react'), view: view ?? '', version: report?.react?.version ?? '' };
    $('status').textContent = this.panel.text(`panel.${status}`, params);
    $('status').dataset.status = status;
  }
}

new ReactPanel(createPanelBridge(window, PREFIX)).start();
