import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import SizeInputBar from './SizeInputBar.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

async function mounted() {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  const result = await mountWithShell(SizeInputBar, { mount: { props: { subscribe } } });
  current = result.wrapper;
  return { ...result, push: (message: CoreToShell) => push(message) };
}

describe('the free size input', () => {
  it('opens on the command with the active view size and sends 500x800', async () => {
    const { wrapper, mock, push } = await mounted();
    expect(wrapper.find('[data-testid="size-input"]').exists()).toBe(false);
    push({ type: 'uiCommand', id: 'view.focus_address_bar' });
    await settle();
    expect(wrapper.find('[data-testid="size-input"]').exists()).toBe(false);
    push({ type: 'uiCommand', id: 'view.set_size' });
    await settle();
    const field = wrapper.get<HTMLInputElement>('[data-testid="size-input-field"]');
    expect(field.element.value).toBe('393x852');
    await field.setValue('500x800');
    await wrapper.get('form').trigger('submit');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'setBreakpoint', view: 1, width: 500, height: 800 });
  });

  it('marks what is no size and sends nothing', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'view.set_size' });
    await settle();
    const before = mock.sent.length;
    await wrapper.get('[data-testid="size-input-field"]').setValue('500 by 800');
    await wrapper.get('form').trigger('submit');
    await settle();
    expect(mock.sent.length).toBe(before);
    expect(wrapper.get('[data-testid="size-input-field"]').attributes('aria-invalid')).toBe('true');
  });

  it('closes on Escape', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'uiCommand', id: 'view.set_size' });
    await settle();
    await wrapper.get('[data-testid="size-input-field"]').trigger('keydown', { key: 'Escape' });
    await settle();
    expect(wrapper.find('[data-testid="size-input"]').exists()).toBe(false);
  });
});
