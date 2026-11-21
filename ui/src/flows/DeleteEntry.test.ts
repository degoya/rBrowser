// "Delete" of one panel entry (RB-090-91): an entry that starts running while the question is
// open drops the question, so its "Delete" cannot be sent past the reason it waits (code review,
// wave 090-w24).
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { mountWithShell, settle } from '../test/mountShell';
import DeleteEntry from './DeleteEntry.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

describe('DeleteEntry', () => {
  it('closes the question when the entry starts running', async () => {
    const { wrapper } = await mountWithShell(DeleteEntry, {
      mount: {
        props: { name: 'cart', command: 'flows.delete', question: 'Delete cart?', confirm: true, blocked: null },
      },
    });
    current = wrapper;
    await wrapper.get('[data-testid="delete"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="delete-confirm"]').exists()).toBe(true);
    await wrapper.setProps({ blocked: 'cart is running or waits to run.' });
    await settle();
    expect(wrapper.find('[data-testid="delete-confirm"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="delete"]').attributes('disabled')).toBeDefined();
    expect(wrapper.emitted('delete')).toBeUndefined();
  });
});
