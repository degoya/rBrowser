import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import { mountWithShell, settle } from '../test/mountShell';
import CellLabel from './CellLabel.vue';
import CellLabelApp from './CellLabelApp.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

describe('the cell label', () => {
  it('shows the label, the size with DPR and the fit zoom', async () => {
    const view = { ...sampleState().views[0]!, scale: 0.5 };
    const { wrapper } = await mountWithShell(CellLabel, { mount: { props: { view, active: true } } });
    current = wrapper;
    expect(wrapper.get('[data-testid="cell-name"]').text()).toBe('Mobile');
    expect(wrapper.get('[data-testid="cell-size"]').text()).toBe('393 × 852 @3x');
    expect(wrapper.get('[data-testid="cell-scale"]').text()).toBe('shown at 50 %');
    expect(wrapper.get('[role="group"]').attributes('aria-label')).toBe('Mobile, 393 × 852 @3x');
    await wrapper.get('[data-testid="cell-size"]').trigger('click');
    expect(wrapper.emitted('pickSize')).toEqual([[1]]);
  });

  it('the label page finds its view by id and makes it active on click', async () => {
    const { wrapper, mock } = await mountWithShell(CellLabelApp, { mount: { props: { viewId: 2 } } });
    current = wrapper;
    await settle();
    expect(wrapper.get('[data-testid="cell-name"]').text()).toBe('Tablet');
    expect(wrapper.get('[data-testid="cell-size"]').text()).toBe('820 × 1180 @2x');
    await wrapper.get('[data-testid="cell-name"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'setActiveView', view: 2 });
  });

  it('shows a named session in its colour and default not at all', async () => {
    const plain = await mountWithShell(CellLabel, {
      mount: { props: { view: sampleState().views[0]!, active: false } },
    });
    expect(plain.wrapper.find('[data-testid="cell-session"]').exists()).toBe(false);
    plain.wrapper.unmount();
    const view = { ...sampleState().views[0]!, session: 'admin' };
    const { wrapper } = await mountWithShell(CellLabel, {
      mount: { props: { view, active: false, sessionColor: '#e5484d' } },
    });
    current = wrapper;
    const badge = wrapper.get('[data-testid="cell-session"]');
    expect(badge.text()).toContain('Session admin');
    expect(badge.attributes('style')).toContain('color');
  });

  it('says so when the view is gone', async () => {
    const { wrapper } = await mountWithShell(CellLabelApp, { mount: { props: { viewId: 9 } } });
    current = wrapper;
    await settle();
    expect(wrapper.text()).toContain('No view 9');
  });
});
