// RB-090-08: the bars the toolbar's page opens over its strip (a size, a custom size, a session,
// basic auth, a bookmark, the sitemap, a layout preset) pass axe-core's WCAG 2.2 A and AA rules
// and the keyboard checks once open.

import type { VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import type { Component } from 'vue';
import EnvironmentAuthBar from '../basic-auth/EnvironmentAuthBar.vue';
import { AUTH_COMMAND } from '../basic-auth/basicAuth';
import BookmarkBar from '../bookmarks/BookmarkBar.vue';
import { ADD_COMMAND } from '../bookmarks/bookmarks';
import SizeInputBar from '../breakpoint_keys/SizeInputBar.vue';
import { OPEN_COMMAND as SIZE_COMMAND } from '../breakpoint_keys/size';
import type { CoreToShell } from '../ipc/protocol';
import SavePresetBar from '../layout/SavePresetBar.vue';
import { SAVE_PRESET_COMMAND } from '../layout/savePreset';
import SessionBar from '../sessions/SessionBar.vue';
import { ASSIGN_COMMAND } from '../sessions/sessions';
import SitemapBar from '../sitemap/SitemapBar.vue';
import { PANEL_COMMAND } from '../sitemap/sitemap';
import { axeViolations } from '../test/axe';
import { unnamedFocusables, unreachableControls } from '../test/keyboard';
import { mountWithShell, settle } from '../test/mountShell';
import NewSizeBar from '../toolbar/NewSizeBar.vue';
import { CUSTOM_SIZE_COMMAND } from '../toolbar/addView';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const BARS: [string, Component, string][] = [
  ['size input', SizeInputBar, SIZE_COMMAND],
  ['custom size', NewSizeBar, CUSTOM_SIZE_COMMAND],
  ['session', SessionBar, ASSIGN_COMMAND],
  ['basic auth', EnvironmentAuthBar, AUTH_COMMAND],
  ['bookmark', BookmarkBar, ADD_COMMAND],
  ['sitemap', SitemapBar, PANEL_COMMAND],
  ['save preset', SavePresetBar, SAVE_PRESET_COMMAND],
];

describe('axe-core over the toolbar bars', () => {
  it.each(BARS)('finds no WCAG violation in the open %s bar', async (_name, bar, command) => {
    let push: (message: CoreToShell) => void = () => undefined;
    const subscribe = (listener: (message: CoreToShell) => void) => {
      push = listener;
      return () => undefined;
    };
    const mounted = await mountWithShell(bar, { mount: { props: { subscribe } } });
    current = mounted.wrapper;
    await settle();
    push({ type: 'uiCommand', id: command });
    await settle();
    expect(current.element.textContent?.trim() ?? '').not.toBe('');
    expect(await axeViolations()).toEqual([]);
    expect(unreachableControls()).toEqual([]);
    expect(unnamedFocusables()).toEqual([]);
  });
});
