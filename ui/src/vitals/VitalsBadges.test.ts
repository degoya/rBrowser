import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import type { ViewMetrics } from '../ipc/protocol';
import CellLabelApp from '../cells/CellLabelApp.vue';
import { mountWithShell, settle } from '../test/mountShell';
import VitalsBadges from './VitalsBadges.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const metrics: ViewMetrics = {
  view: 1,
  vitals: [
    { metric: 'lcp', value: 1840, rating: 'good', highlight: true },
    { metric: 'cls', value: 0.31, rating: 'poor', highlight: true },
    { metric: 'inp', unavailable: 'pending' },
    { metric: 'fcp', value: 2100, rating: 'needs-improvement' },
    { metric: 'ttfb', unavailable: 'unsupported' },
  ],
  resources: { cpuPercent: 12.4, memoryMb: 48 },
};

describe('the Web Vitals badges', () => {
  it('show each measured vital with shape, name and value, and the rating in words', async () => {
    const { wrapper } = await mountWithShell(VitalsBadges, { mount: { props: { metrics } } });
    current = wrapper;
    const lcp = wrapper.get('[data-testid="vital-lcp"]');
    expect(lcp.text()).toContain('●');
    expect(lcp.text()).toContain('LCP');
    expect(lcp.text()).toContain('1.84 s');
    expect(lcp.attributes('aria-label')).toContain('LCP 1.84 s: good');
    const cls = wrapper.get('[data-testid="vital-cls"]');
    expect(cls.text()).toContain('▲');
    expect(cls.attributes('aria-label')).toContain('CLS 0.31: poor');
    expect(wrapper.get('[data-testid="vital-fcp"]').text()).toContain('■');
    expect(wrapper.find('[data-testid="vital-inp"]').exists()).toBe(false);
    expect(lcp.attributes('title')).toContain('INP: not measured yet');
    expect(lcp.attributes('title')).toContain('TTFB: n/a in this engine');
  });

  it('marks the element behind a vital on click, and not without one', async () => {
    const { wrapper } = await mountWithShell(VitalsBadges, { mount: { props: { metrics } } });
    current = wrapper;
    await wrapper.get('[data-testid="vital-cls"]').trigger('click');
    await wrapper.get('[data-testid="vital-fcp"]').trigger('click');
    expect(wrapper.emitted('highlight')).toEqual([[1, 'cls']]);
  });

  it('shows the resources and warns in shape and words above a threshold', async () => {
    const calm = await mountWithShell(VitalsBadges, { mount: { props: { metrics } } });
    current = calm.wrapper;
    const reading = calm.wrapper.get('[data-testid="vital-resources"]');
    expect(reading.text()).toBe('CPU 12 % · 48 MB');
    expect(reading.attributes('tabindex')).toBe('0');
    calm.wrapper.unmount();
    const hot = { ...metrics, resources: { cpuPercent: 80, memoryMb: 640, ramWarn: true } };
    const warned = await mountWithShell(VitalsBadges, { mount: { props: { metrics: hot } } });
    current = warned.wrapper;
    const badge = warned.wrapper.get('[data-testid="vital-resources"]');
    expect(badge.text()).toContain('▲');
    expect(badge.text()).toContain('high');
    expect(badge.attributes('title')).toContain('The memory is above the warning threshold.');
  });

  it('names a throttled view and a soft segment', async () => {
    const soft = { ...metrics, soft: true, throttle: { state: 'cpuLimited', fps: 30 } as const };
    const { wrapper } = await mountWithShell(VitalsBadges, { mount: { props: { metrics: soft } } });
    current = wrapper;
    expect(wrapper.get('[data-testid="vital-throttle"]').text()).toBe('throttled (30 fps)');
    expect(wrapper.get('[data-testid="vital-cls"]').attributes('title')).toContain('only CLS and INP');
  });

  it('shows nothing without metrics', async () => {
    const { wrapper } = await mountWithShell(VitalsBadges, { mount: { props: { metrics: null } } });
    current = wrapper;
    expect(wrapper.find('[data-testid="vitals-badges"]').exists()).toBe(false);
  });

  it('the label page shows its view and asks the core to mark the element', async () => {
    const state = { ...sampleState(), metrics: [{ ...metrics, view: 2 }] };
    const { wrapper, mock } = await mountWithShell(CellLabelApp, {
      mock: { state },
      mount: { props: { viewId: 2 } },
    });
    current = wrapper;
    await settle();
    await wrapper.get('[data-testid="vital-lcp"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'highlightVital', view: 2, metric: 'lcp' });
  });

  it('colours the label of a view above a warning threshold', async () => {
    const hot = { ...metrics, view: 2, resources: { memoryMb: 900, ramWarn: true } };
    const { wrapper } = await mountWithShell(CellLabelApp, {
      mock: { state: { ...sampleState(), metrics: [hot] } },
      mount: { props: { viewId: 2 } },
    });
    current = wrapper;
    await settle();
    expect(wrapper.get('main').classes()).toContain('bg-error/10');
  });
});
