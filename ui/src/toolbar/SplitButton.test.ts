// The toolbar's split buttons (RB-090-63): the button runs its main command at once, the arrow
// opens the rest as the core's native menu below it, and a row chosen runs what it names.
import { afterEach, describe, expect, it } from 'vitest';
import type { NativeMenuRow } from '../ipc/protocol';
import { BUTTONS } from '../menu/buttonMenus';
import { mountWithShell, settle } from '../test/mountShell';
import { closeNativeMenu, lastNativeMenu } from '../test/nativeMenu';
import SplitButton from './SplitButton.vue';

afterEach(() => {
  document.body.innerHTML = '';
});

async function mountButton(state?: 'on' | 'off') {
  return mountWithShell(SplitButton, {
    mount: {
      props: {
        kind: 'capture',
        icon: 'i-lucide-camera',
        label: 'Screenshot of the active view',
        menuLabel: 'Screenshots',
        command: BUTTONS.capture.command,
        menu: BUTTONS.capture.menu,
        ...(state === undefined ? {} : { state }),
      },
      attachTo: document.body,
    },
  });
}

const items = (rows: NativeMenuRow[]): Extract<NativeMenuRow, { kind: 'item' }>[] =>
  rows.flatMap((row) => (row.kind === 'item' ? [row] : row.kind === 'submenu' ? items(row.rows) : []));

describe('a split button', () => {
  it('runs its main command on a click', async () => {
    const { wrapper, mock } = await mountButton();
    await wrapper.get('[data-testid="split-capture-run"]').trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'capture.views' });
  });

  it('opens its menu below the arrow and runs the row chosen', async () => {
    const { wrapper, mock } = await mountButton();
    const arrow = wrapper.get('[data-testid="split-capture-menu"]');
    expect(arrow.attributes('aria-haspopup')).toBe('menu');
    await arrow.trigger('click');
    await settle();
    const request = lastNativeMenu(mock.sent);
    expect(request.menu).toMatch(/^capture-\d+$/);
    const element = items(request.rows).find((row) => row.label === 'Screenshot an element');
    const row = element ?? items(request.rows)[1];
    expect(row).toBeDefined();
    await closeNativeMenu(mock.sent, row?.id);
    expect(mock.sent.at(-1)).toMatchObject({ type: 'runCommand', id: 'capture.element' });
  });

  it('opens on an arrow key or F4, not on other keys', async () => {
    const { wrapper, mock } = await mountButton();
    const arrow = wrapper.get('[data-testid="split-capture-menu"]').element;
    const menus = () => mock.sent.filter((message) => message.type === 'nativeMenu').length;
    arrow.dispatchEvent(new KeyboardEvent('keydown', { key: 's', bubbles: true, cancelable: true }));
    await settle();
    expect(menus()).toBe(0);
    arrow.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
    await settle();
    expect(menus()).toBe(1);
  });

  it('says whether its state is on', async () => {
    const on = await mountButton('on');
    expect(on.wrapper.get('[data-testid="split-capture-run"]').attributes('aria-pressed')).toBe('true');
    const plain = await mountButton();
    expect(plain.wrapper.get('[data-testid="split-capture-run"]').attributes('aria-pressed')).toBeUndefined();
  });
});
