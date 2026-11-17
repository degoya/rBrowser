// Mounts a Nuxt UI panel (RB-080-83): the prelude first (storage stand-in, texts.js), Nuxt UI as
// a Vue plugin, the bundled Lucide icons only - the frame has no network, so the Iconify API is
// never asked -, the app theme from `ui.theme` and the text size from `ui.textScale` (RB-090-39),
// and the panel's link to the panels window, provided to every component (`usePanel()`).
import './prelude';
import ui from '@nuxt/ui/vue-plugin';
import { _api as iconifyApi } from '@iconify/vue';
import { createApp, inject, type Component, type InjectionKey } from 'vue';
import { createPanel, type Panel, type PanelOptions } from './panel';
import { themeFollower } from './theme';

export const PANEL_KEY: InjectionKey<Panel> = Symbol('panel');

/** The panel's link, inside a panel component. */
export function usePanel(): Panel {
  const panel = inject(PANEL_KEY);
  if (!panel) throw new Error('usePanel() outside a mounted panel');
  return panel;
}

/** Mounts `root` on `#app`: the panel's link, already listening, and what takes it down. */
export function mountPanel(root: Component, options: Omit<PanelOptions, 'onTheme'>, selector = '#app'): { panel: Panel; unmount: () => void } {
  iconifyApi.setFetch(() => Promise.reject(new Error('icons are bundled')));
  const theme = themeFollower(document.documentElement, (query) => window.matchMedia(query));
  const panel = createPanel({ ...options, onTheme: theme });
  const app = createApp(root).provide(PANEL_KEY, panel).use(ui);
  app.mount(selector);
  const stop = panel.start();
  return {
    panel,
    unmount: () => {
      stop();
      app.unmount();
    },
  };
}
