// RB-090-08: the shell's surfaces in a filled state pass axe-core's WCAG 2.2 A and AA rules -
// every category and tab of the settings (the shortcut editor included), the open command
// palette, the which-key overlay and the findings with a finished run (its "Run again" controls
// open, RB-094-17).

import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import FindingsPage from '../findings/FindingsPage.vue';
import { sampleMessages, sampleProgress, sampleRun } from '../findings/testing';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import type { CoreToShell } from '../ipc/protocol';
import CommandPalette from '../palette/CommandPalette.vue';
import { PALETTE_COMMAND } from '../palette/constants';
import { PALETTE_CLIENT_KEY } from '../palette/palette';
import { createPaletteCore } from '../palette/testCore';
import SettingsPage from '../settings/SettingsPage.vue';
import { mountPage, settle } from '../settings/test/mountPage';
import { createShell, SHELL_KEY } from '../state/shell';
import { axeViolations } from '../test/axe';
import { cleanup as cleanupBoard, setup as setupBoard } from '../test/canvasBoard';
import { unnamedFocusables, unreachableControls } from '../test/keyboard';
import { mountWithShell } from '../test/mountShell';
import WhichKeyBar from '../whichkey/WhichKeyBar.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

/** A subscription the test pushes core events into. */
function events() {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  return { subscribe, push: (message: CoreToShell) => push(message) };
}

describe('axe-core over the shell surfaces', () => {
  it('finds no WCAG violation in any category or tab of the settings', async () => {
    const { wrapper } = await mountPage(SettingsPage, { routeKey: null });
    current = wrapper;
    const found: Record<string, unknown> = {};
    const categories = wrapper.findAll('[data-category]').map((button) => button.attributes('data-category'));
    expect(categories.length).toBeGreaterThan(10);
    for (const id of categories) {
      await wrapper.get(`[data-category="${id}"]`).trigger('click');
      await settle();
      const tabs = wrapper.findAll('[data-testid="settings-tabs"] button[role="tab"]');
      for (const [index, tab] of (tabs.length > 0 ? tabs : [null]).entries()) {
        await tab?.trigger('mousedown', { button: 0 });
        await settle();
        const violations = { axe: await axeViolations(), unreachable: unreachableControls(), unnamed: unnamedFocusables() };
        if (Object.values(violations).some((list) => list.length > 0)) found[`${id}/${index}`] = violations;
      }
    }
    expect(found).toEqual({});
    // A give-up deadline, not a speed bound: axe over every category and tab of the settings, in a
    // loaded check beside the other npm stages, took over 30 s.
  }, 180_000);

  it('finds no WCAG violation in the open command palette', async () => {
    const core = createPaletteCore();
    const shell = createShell(core.client, {
      root: document.createElement('html'),
      matchMedia: () => ({ matches: false, addEventListener: () => undefined, removeEventListener: () => undefined }),
    });
    await shell.start();
    const { subscribe, push } = events();
    current = mount(CommandPalette, {
      props: { subscribe },
      attachTo: document.body,
      global: {
        plugins: [ui, createShellI18n()],
        provide: { [SHELL_KEY]: shell, [PALETTE_CLIENT_KEY]: core.client },
      },
    });
    push({ type: 'uiCommand', id: PALETTE_COMMAND });
    await settle();
    expect(document.querySelector('[role="combobox"]')).not.toBeNull();
    expect(await axeViolations()).toEqual([]);
    expect(unreachableControls()).toEqual([]);
    expect(unnamedFocusables()).toEqual([]);
  });

  it('finds no WCAG violation in the which-key overlay', async () => {
    const { subscribe, push } = events();
    current = mount(WhichKeyBar, {
      props: { subscribe },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    push({
      type: 'whichKey',
      typed: 'Ctrl+Shift+Space',
      entries: [
        { kind: 'group', key: 'S', group: 'sync', titleKey: 'commands.group.sync' },
        { kind: 'command', key: ',', id: 'settings.open', titleKey: 'commands.settings.open' },
      ],
    });
    await nextTick();
    expect(document.querySelector('[data-testid="which-key"]')).not.toBeNull();
    expect(await axeViolations()).toEqual([]);
    expect(unreachableControls()).toEqual([]);
    expect(unnamedFocusables()).toEqual([]);
  });

  it('finds no WCAG violation in the findings with a finished run', async () => {
    const { subscribe, push } = events();
    const client = new IpcClient({ send: async () => ({ type: 'ack' }), subscribe: () => () => undefined });
    const mounted = await mountWithShell(FindingsPage, { mount: { props: { subscribe, client } } });
    current = mounted.wrapper;
    push({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    await settle();
    expect(document.querySelector('[data-testid="finding-a"]')).not.toBeNull();
    expect(await axeViolations()).toEqual([]);
    expect(unreachableControls()).toEqual([]);
    expect(unnamedFocusables()).toEqual([]);
  });

  it('finds no WCAG violation in the findings with the details open to run checks again (RB-094-17)', async () => {
    const { subscribe, push } = events();
    const client = new IpcClient({ send: async () => ({ type: 'ack' }), subscribe: () => () => undefined });
    const mounted = await mountWithShell(FindingsPage, { mount: { props: { subscribe, client } } });
    current = mounted.wrapper;
    push({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    push(sampleProgress({ finished: true, jobs: sampleProgress().jobs.map((job) => ({ ...job, state: 'done' as const, findings: 0 })) }));
    await settle();
    document.querySelector<HTMLElement>('[data-testid="progress-details"]')?.click();
    await settle();
    document.querySelector('[data-testid="progress-seo"]')?.closest('button')?.click();
    await settle();
    expect(document.querySelector('[data-testid="rerun-selected"]')).not.toBeNull();
    expect(document.querySelector('[data-testid="rerun-seo"]')).not.toBeNull();
    expect(await axeViolations()).toEqual([]);
    expect(unreachableControls()).toEqual([]);
    expect(unnamedFocusables()).toEqual([]);
  });

  it('finds no WCAG violation in the findings while a run is going', async () => {
    const { subscribe, push } = events();
    const client = new IpcClient({ send: async () => ({ type: 'ack' }), subscribe: () => () => undefined });
    const mounted = await mountWithShell(FindingsPage, { mount: { props: { subscribe, client } } });
    current = mounted.wrapper;
    push(sampleProgress());
    await settle();
    expect(document.querySelector('[data-testid="progress"]')).not.toBeNull();
    expect(await axeViolations()).toEqual([]);
    expect(unreachableControls()).toEqual([]);
    expect(unnamedFocusables()).toEqual([]);
  });

  it('finds no WCAG violation on the canvas board with its labels and badges', async () => {
    await setupBoard(true, { pageZoom: 125, firefoxMissing: true });
    try {
      expect(document.querySelector('[data-testid^="board-label-"]')).not.toBeNull();
      expect(await axeViolations()).toEqual([]);
      expect(unreachableControls()).toEqual([]);
      expect(unnamedFocusables()).toEqual([]);
    } finally {
      cleanupBoard();
    }
  });
});
