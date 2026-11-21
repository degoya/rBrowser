import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell, NativeMenuRow, ShellToCore } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import AddViewMenu from './AddViewMenu.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
  document.body.innerHTML = '';
});

async function mounted(hidden: string[]) {
  // The button listens twice: for its command and for its native menu's close.
  const listeners: ((message: CoreToShell) => void)[] = [];
  const push = (message: CoreToShell) => listeners.forEach((listener) => listener(message));
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.push(listener);
    return () => undefined;
  };
  const result = await mountWithShell(AddViewMenu, { mount: { props: { subscribe } } });
  current = result.wrapper;
  result.mock.state.layout = { ...result.mock.state.layout, hidden };
  result.mock.emit({ type: 'state', ...structuredClone(result.mock.state) });
  await settle();
  return { ...result, push: (message: CoreToShell) => push(message) };
}

type MenuRequest = Extract<ShellToCore, { type: 'nativeMenu' }>;
const items = (rows: NativeMenuRow[]): Extract<NativeMenuRow, { kind: 'item' }>[] =>
  rows.flatMap((row) => (row.kind === 'item' ? [row] : row.kind === 'submenu' ? items(row.rows) : []));

describe('the "+" button', () => {
  // RB-080-46: the toolbar is a strip 44 px tall - a menu it draws itself is cut off there. Since
  // RB-094-20 the rows are the shell's and the core shows them natively under the button, in
  // front of the views, with the devices and engines after the breakpoints.
  it('names the hidden count and opens its rows as a native menu under it', async () => {
    const { wrapper, mock } = await mounted(['desktop', 'tablet']);
    const button = wrapper.get('[data-testid="add-view"]');
    expect(button.attributes('aria-label')).toBe('Add view (2 hidden)');
    expect(button.attributes('data-hidden')).toBe('2');
    const anchor = button.element.parentElement as HTMLElement;
    anchor.getBoundingClientRect = () =>
      ({ left: 96.4, bottom: 38.6, top: 6, right: 128, width: 32, height: 32, x: 96.4, y: 6 }) as DOMRect;
    await button.trigger('click');
    await settle();
    const request = mock.sent.at(-1) as MenuRequest;
    expect(request).toMatchObject({ type: 'nativeMenu', anchor: { x: 96, y: 6, width: 32, height: 32 } });
    expect(request.menu).toMatch(/^add-\d+$/);
    const labels = items(request.rows).map((row) => row.label);
    expect(labels).toContain('Add all');
    expect(labels).toContain('Download adb');
    expect(document.body.querySelector('[role="menu"]'), "no menu in the toolbar's own page").toBeNull();
  });

  it('runs the row chosen through the registry', async () => {
    const { wrapper, mock, push } = await mounted(['desktop', 'tablet']);
    await wrapper.get('[data-testid="add-view"]').trigger('click');
    await settle();
    const request = mock.sent.at(-1) as MenuRequest;
    const all = items(request.rows).find((row) => row.label === 'Add all');
    push({ type: 'nativeMenuClosed', menu: request.menu, item: all?.id });
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'view.add', args: { breakpoint: 'all' } });
  });

  it('opens the menu on the command too (Leader V +)', async () => {
    const { wrapper, mock, push } = await mounted([]);
    expect(wrapper.get('[data-testid="add-view"]').attributes('aria-label')).toBe('Add view');
    push({ type: 'uiCommand', id: 'view.add' });
    await settle();
    expect(mock.sent.at(-1)).toMatchObject({ type: 'nativeMenu' });
  });
});
