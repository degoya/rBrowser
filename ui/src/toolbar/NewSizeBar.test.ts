import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import NewSizeBar from './NewSizeBar.vue';

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
  const result = await mountWithShell(NewSizeBar, { mount: { props: { subscribe } } });
  current = result.wrapper;
  return { ...result, push: (message: CoreToShell) => push(message) };
}

describe('the new size bar', () => {
  it('opens on the command and adds a view of the typed size and name', async () => {
    const { wrapper, mock, push } = await mounted();
    expect(wrapper.find('[data-testid="new-size"]').exists()).toBe(false);
    push({ type: 'uiCommand', id: 'view.add_size.custom' });
    await settle();
    expect(wrapper.find('[data-testid="new-size"]').exists()).toBe(true);
    await wrapper.get('[data-testid="new-size-field"]').setValue('1440 x 900 @2');
    await wrapper.get('[data-testid="new-size-name"]').setValue(' wide ');
    await wrapper.get('form').trigger('keydown', { key: 'Enter' });
    await settle();
    expect(mock.sent.at(-1)).toEqual({
      type: 'runCommand',
      id: 'view.add_size',
      args: { width: 1440, height: 900, dpr: 2, name: 'wide' },
    });
    // The mock core refuses the command: the message reaches the status bar, the bar stays.
    expect(mock.reports.length).toBeGreaterThan(0);
    expect(wrapper.find('[data-testid="new-size"]').exists()).toBe(true);
  });

  it('refuses a bad size or name and closes with Escape', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'view.add_size.custom' });
    await settle();
    const before = mock.sent.length;
    await wrapper.get('[data-testid="new-size-field"]').setValue('wide');
    await wrapper.get('form').trigger('keydown', { key: 'Enter' });
    await settle();
    expect(wrapper.get('[data-testid="new-size-field"]').attributes('aria-invalid')).toBe('true');
    await wrapper.get('[data-testid="new-size-field"]').setValue('800x600');
    await wrapper.get('[data-testid="new-size-name"]').setValue('a b');
    await wrapper.get('form').trigger('keydown', { key: 'Enter' });
    await settle();
    expect(mock.sent.length).toBe(before);
    await wrapper.get('form').trigger('keydown', { key: 'Escape' });
    await settle();
    expect(wrapper.find('[data-testid="new-size"]').exists()).toBe(false);
  });

  // RB-080-46: the bar has two text fields and no submit button, so the browser's implicit
  // submission never ran - Enter did nothing. The key itself is what the bar hears now.
  it('adds the view on Enter in either field', async () => {
    for (const field of ['new-size-field', 'new-size-name']) {
      const { wrapper, mock, push } = await mounted();
      push({ type: 'uiCommand', id: 'view.add_size.custom' });
      await settle();
      await wrapper.get('[data-testid="new-size-field"]').setValue('1000x1000');
      await wrapper.get('[data-testid="new-size-name"]').setValue('testsetestt');
      await wrapper.get(`[data-testid="${field}"]`).trigger('keydown', { key: 'Enter' });
      await settle();
      expect(mock.sent.at(-1)).toEqual({
        type: 'runCommand',
        id: 'view.add_size',
        args: { width: 1000, height: 1000, name: 'testsetestt' },
      });
      wrapper.unmount();
      current = null;
    }
  });

  it('says why an input was refused: the size, the name, the core', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'uiCommand', id: 'view.add_size.custom' });
    await settle();
    const reason = () => wrapper.get('[data-testid="new-size-hint"]').attributes('data-reason');
    expect(reason()).toBeUndefined();
    await wrapper.get('[data-testid="new-size-field"]').setValue('wide');
    await wrapper.get('[data-testid="new-size-field"]').trigger('keydown', { key: 'Enter' });
    await settle();
    expect(reason()).toBe('size');
    await wrapper.get('[data-testid="new-size-field"]').setValue('800x600');
    await wrapper.get('[data-testid="new-size-name"]').setValue('a b');
    await wrapper.get('[data-testid="new-size-name"]').trigger('keydown', { key: 'Enter' });
    await settle();
    expect(reason()).toBe('name');
    expect(wrapper.get('[data-testid="new-size-name"]').attributes('aria-invalid')).toBe('true');
    // The mock core refuses the command: its words stand at the bar.
    await wrapper.get('[data-testid="new-size-name"]').setValue('wide');
    await wrapper.get('[data-testid="new-size-name"]').trigger('keydown', { key: 'Enter' });
    await settle();
    expect(reason()).toBe('refused');
    // Typing again takes the reason away.
    await wrapper.get('[data-testid="new-size-field"]').setValue('800x601');
    expect(reason()).toBeUndefined();
  });

  it('does not open on the picker\'s command', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'uiCommand', id: 'view.add_size' });
    await settle();
    expect(wrapper.find('[data-testid="new-size"]').exists()).toBe(false);
  });
});
