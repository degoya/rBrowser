// RB-090-08: the bars the toolbar's page opens over its strip (a size, a custom size, a session,
// basic auth, a bookmark, the sitemap, a layout preset) pass axe-core's WCAG 2.2 A and AA rules
// and the keyboard checks once open; so does the sign-in bar a site's or a proxy's challenge
// opens (RB-099-21), with every part of its question shown.

import type { VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import type { Component } from 'vue';
import AuthPromptBar from '../basic-auth/AuthPromptBar.vue';
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

const command = (id: string): CoreToShell => ({ type: 'uiCommand', id });

/** The core's question that opens the sign-in bar: realm, waiting views, a save, refused, plain http. */
const AUTH_PROMPT: CoreToShell = {
  type: 'authPrompt',
  id: 4,
  origin: 'http://dev.example.test:80',
  realm: 'Members',
  proxy: false,
  views: [1, 3],
  environment: 'development',
  refused: true,
  insecure: true,
};

/** Each bar with the message that opens it. */
const BARS: [string, Component, CoreToShell][] = [
  ['size input', SizeInputBar, command(SIZE_COMMAND)],
  ['custom size', NewSizeBar, command(CUSTOM_SIZE_COMMAND)],
  ['session', SessionBar, command(ASSIGN_COMMAND)],
  ['basic auth', EnvironmentAuthBar, command(AUTH_COMMAND)],
  ['sign-in', AuthPromptBar, AUTH_PROMPT],
  ['bookmark', BookmarkBar, command(ADD_COMMAND)],
  ['sitemap', SitemapBar, command(PANEL_COMMAND)],
  ['save preset', SavePresetBar, command(SAVE_PRESET_COMMAND)],
];

describe('axe-core over the toolbar bars', () => {
  it.each(BARS)('finds no WCAG violation in the open %s bar', async (_name, bar, opener) => {
    let push: (message: CoreToShell) => void = () => undefined;
    const subscribe = (listener: (message: CoreToShell) => void) => {
      push = listener;
      return () => undefined;
    };
    const mounted = await mountWithShell(bar, { mount: { props: { subscribe } } });
    current = mounted.wrapper;
    await settle();
    push(opener);
    await settle();
    expect(current.element.textContent?.trim() ?? '').not.toBe('');
    expect(await axeViolations()).toEqual([]);
    expect(unreachableControls()).toEqual([]);
    expect(unnamedFocusables()).toEqual([]);
  });

  it('finds no WCAG violation in the sign-in bar with a problem said', async () => {
    let push: (message: CoreToShell) => void = () => undefined;
    const subscribe = (listener: (message: CoreToShell) => void) => {
      push = listener;
      return () => undefined;
    };
    const mounted = await mountWithShell(AuthPromptBar, { mount: { props: { subscribe } } });
    current = mounted.wrapper;
    push(AUTH_PROMPT);
    await settle();
    await current.get('[data-testid="auth-prompt-save"]').trigger('click');
    await current.get('[data-testid="auth-prompt-password"]').setValue('pw');
    await current.get('[data-testid="auth-prompt"]').trigger('submit');
    await settle();
    expect(current.get('[role="alert"]').text()).not.toBe('');
    expect(await axeViolations()).toEqual([]);
  });
});
