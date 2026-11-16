import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import AddViewMenu from './AddViewMenu.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
  document.body.innerHTML = '';
});

async function mounted(hidden: string[]) {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  const result = await mountWithShell(AddViewMenu, { mount: { props: { subscribe } } });
  current = result.wrapper;
  result.mock.state.layout = { ...result.mock.state.layout, hidden };
  result.mock.emit({ type: 'state', ...structuredClone(result.mock.state) });
  await settle();
  return { ...result, push: (message: CoreToShell) => push(message) };
}

describe('the "+" button', () => {
  // RB-080-46: the toolbar is a strip 44 px tall - a menu it draws itself is cut off there. The
  // button names its place and the core shows the native add menu under it, in front of the views.
  it('names the hidden count and asks the core for the add menu under it', async () => {
    const { wrapper, mock } = await mounted(['desktop', 'tablet']);
    const button = wrapper.get('[data-testid="add-view"]');
    expect(button.attributes('aria-label')).toBe('Add view (2 hidden)');
    expect(button.attributes('data-hidden')).toBe('2');
    const anchor = button.element.parentElement as HTMLElement;
    anchor.getBoundingClientRect = () =>
      ({ left: 96.4, bottom: 38.6, top: 6, right: 128, width: 32, height: 32, x: 96.4, y: 6 }) as DOMRect;
    await button.trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'addViewMenu', x: 96, y: 39 });
    expect(document.body.querySelector('[role="menu"]'), 'no menu in the toolbar\'s own page').toBeNull();
  });

  it('opens the menu on the command too (Leader V +)', async () => {
    const { wrapper, mock, push } = await mounted([]);
    expect(wrapper.get('[data-testid="add-view"]').attributes('aria-label')).toBe('Add view');
    push({ type: 'uiCommand', id: 'view.add' });
    await settle();
    expect(mock.sent.at(-1)).toMatchObject({ type: 'addViewMenu' });
  });
});
