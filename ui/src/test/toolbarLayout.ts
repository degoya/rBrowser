// The toolbar with a layout, for tests (RB-080-52). happy-dom lays nothing out, so the widths are
// the owner's toolbar of 2026-11-14 as numbers: each group reports its own, the row the window's.
import { afterAll, afterEach, beforeAll } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { NativeMenuRow, ShellToCore } from '../ipc/protocol';
import { sampleState } from '../ipc/mockState';
import { ADDRESS_MIN, GAP, ICON_WIDTH, PADDING } from '../toolbar/fit';
import ShellToolbar from '../toolbar/ShellToolbar.vue';
import { mountWithShell, settle } from './mountShell';
import { closeNativeMenu, lastNativeMenu } from './nativeMenu';

/** A pulldown with its value (`PulldownButton.vue`, RB-090-45); 32 px as an icon. */
const PULLDOWN = 160;
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
  settings: 28,
  // The split buttons (RB-090-63): an icon button and its arrow each; Screenshot and Video in one
  // group.
  devtools: 52,
  capture: 108,
  checks: 52,
  // The plugin panels' switch (RB-080-79), an icon button; off by default (RB-090-10).
  panels: 28,
  // The portal's button (RB-090-62), an icon button as wide as the theme's.
  portal: 32,
};
let windowWidth = 0;

const original = {
  offsetWidth: Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth'),
  clientWidth: Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientWidth'),
};

let current: VueWrapper | null = null;
/** What the toolbar mounted last sent the core. */
let sent: ShellToCore[] = [];

/** Installs the widths for the tests of the calling file and unmounts after each. */
export function useToolbarLayout(): void {
  beforeAll(() => {
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
      configurable: true,
      get(this: HTMLElement) {
        if (this.dataset.pulldown !== undefined) {
          return this.closest('[data-state="compact"]') === null ? PULLDOWN : ICON_WIDTH;
        }
        const id = this.dataset.toolbarGroup;
        if (id === undefined || this.childElementCount === 0) {
          return 0;
        }
        const full = WIDTHS[id] ?? 0;
        return this.dataset.state === 'compact' ? full - (PULLDOWN - ICON_WIDTH) : full;
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
  sent = mounted.mock.sent;
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

const OVERFLOW_BUTTON = '[data-testid="toolbar-overflow-button"]';

/**
 * The overflow menu's rows as a click on its button asks the core for them (RB-090-45), and the
 * menu closed again without a choice; none without the menu.
 */
export async function menuRows(wrapper: VueWrapper): Promise<NativeMenuRow[]> {
  const button = wrapper.find(OVERFLOW_BUTTON);
  if (!button.exists()) {
    return [];
  }
  await button.trigger('click');
  await settle();
  const { rows } = lastNativeMenu(sent);
  await closeNativeMenu(sent);
  return rows;
}

/** The texts of the menu's rows, its headings included (a hidden pulldown's name). */
export async function menuLabels(wrapper: VueWrapper): Promise<string[]> {
  return (await menuRows(wrapper)).flatMap((row) => (row.kind === 'separator' ? [] : [row.label]));
}

/** Opens the overflow menu and chooses the row `label` there, as the core tells it. */
export async function chooseInMenu(wrapper: VueWrapper, label: string): Promise<NativeMenuRow | undefined> {
  await wrapper.get(OVERFLOW_BUTTON).trigger('click');
  await settle();
  const row = lastNativeMenu(sent).rows.find((candidate) => candidate.kind === 'item' && candidate.label === label);
  await closeNativeMenu(sent, row?.kind === 'item' && row.disabled !== true ? row.id : undefined);
  await settle();
  return row;
}
