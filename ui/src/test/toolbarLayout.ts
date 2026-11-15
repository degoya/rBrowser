// The toolbar with a layout, for tests (RB-080-52). happy-dom lays nothing out, so the widths are
// the owner's toolbar of 2026-11-14 as numbers: each group reports its own, the row the window's.
import { afterAll, afterEach, beforeAll } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import { ADDRESS_MIN, GAP, ICON_WIDTH, PADDING } from '../toolbar/fit';
import ShellToolbar from '../toolbar/ShellToolbar.vue';
import { mountWithShell, settle } from './mountShell';

const SELECT = 160;
/** The groups' full widths; a group that is not named or renders nothing has none. */
const WIDTHS: Record<string, number> = {
  logo: 96,
  navigation: 88,
  sync: 76,
  layout: 392,
  environment: 176,
  bookmarks: 190,
  emulation: 142,
  browsersync: 150,
  'minimap-end': 144,
  analyze: 28,
  settings: 28,
  // The breakpoint bar's switch (RB-080-60), an icon button like analyse and settings.
  footer: 28,
  // rBrowser's theme menu (RB-080-95, RB-090-10), an icon select as wide as the overflow menu's.
  theme: 32,
  // The plugin panels' switch (RB-080-79), an icon button; off by default (RB-090-10).
  panels: 28,
};
let windowWidth = 0;

const original = {
  offsetWidth: Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth'),
  clientWidth: Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientWidth'),
};

let current: VueWrapper | null = null;

/** Installs the widths for the tests of the calling file and unmounts after each. */
export function useToolbarLayout(): void {
  beforeAll(() => {
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
      configurable: true,
      get(this: HTMLElement) {
        if (this instanceof HTMLSelectElement) {
          return this.closest('[data-state="compact"]') === null ? SELECT : ICON_WIDTH;
        }
        const id = this.dataset.toolbarGroup;
        if (id === undefined || this.childElementCount === 0) {
          return 0;
        }
        const full = WIDTHS[id] ?? 0;
        return this.dataset.state === 'compact' ? full - (SELECT - ICON_WIDTH) : full;
      },
    });
    Object.defineProperty(HTMLElement.prototype, 'clientWidth', {
      configurable: true,
      get(this: HTMLElement) {
        return this.getAttribute('role') === 'toolbar' ? windowWidth : 0;
      },
    });
  });

  afterAll(() => {
    for (const name of ['offsetWidth', 'clientWidth'] as const) {
      const before = original[name];
      if (before === undefined) {
        Reflect.deleteProperty(HTMLElement.prototype, name);
      } else {
        Object.defineProperty(HTMLElement.prototype, name, before);
      }
    }
  });

  afterEach(() => {
    current?.unmount();
    current = null;
  });
}

export async function setup(
  width: number,
  settings: Record<string, unknown> = {},
  change: (state: ReturnType<typeof sampleState>) => void = () => undefined,
) {
  windowWidth = width;
  const state = sampleState();
  change(state);
  // The canvas minimap at the toolbar's end.
  state.layout.modes = {
    canvas: { position: 'end', items: [], viewport: { x: 0, y: 0, width: 1000, height: 800 } },
  };
  const mounted = await mountWithShell(ShellToolbar, { mock: { state, settings } });
  current = mounted.wrapper;
  await settle();
  await settle();
  return mounted;
}

export async function resize(width: number): Promise<void> {
  windowWidth = width;
  window.dispatchEvent(new Event('resize'));
  await settle();
  await settle();
}

export function states(wrapper: VueWrapper): Record<string, string> {
  return Object.fromEntries(
    wrapper
      .findAll('[data-toolbar-group]')
      .filter((group) => group.element.childElementCount > 0)
      .map((group) => [group.attributes('data-toolbar-group') ?? '', group.attributes('data-state') ?? '']),
  );
}

/** The row's width as drawn: padding, every group in the flow, the menu, the address bar's floor. */
export function rowWidth(wrapper: VueWrapper): number {
  const groups = wrapper
    .findAll('[data-toolbar-group]')
    .filter((group) => group.attributes('data-state') !== 'hidden')
    .map((group) => (group.element as HTMLElement).offsetWidth)
    .filter((width) => width > 0);
  const menu = wrapper.find('[data-testid="toolbar-overflow"]').exists() ? [ICON_WIDTH] : [];
  const items = [...groups, ...menu, ADDRESS_MIN];
  return PADDING + items.reduce((sum, width) => sum + width, 0) + GAP * (items.length - 1);
}

/** The menu's entries as a press on its button reads them. */
export async function menuLabels(wrapper: VueWrapper): Promise<string[]> {
  const select = wrapper.find('[data-testid="toolbar-overflow-select"]');
  if (!select.exists()) {
    return [];
  }
  await select.trigger('pointerdown');
  return select
    .findAll('option')
    .map((option) => option.text())
    .slice(1);
}
