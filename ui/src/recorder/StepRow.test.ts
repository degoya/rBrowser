// One step of the Recorder panel: a step the recorder kept with the pointer's way names its
// points beside the kind (RB-090-87); a step without one shows nothing for it.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { mountWithShell, settle } from '../test/mountShell';
import type { PathPoint, Step } from './recording';
import StepRow from './StepRow.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

function hover(path?: PathPoint[]): Step {
  const step: Step = {
    id: 2,
    action: { type: 'hover', target: { selectors: [{ kind: 'css', path: '#products' }] } },
    view: 'desktop',
    atMs: 1_200,
  };
  return path === undefined ? step : { ...step, path };
}

async function row(step: Step) {
  const { wrapper } = await mountWithShell(StepRow, {
    mount: { props: { step, index: 1, count: 3 } },
  });
  current = wrapper;
  await settle();
  return wrapper;
}

describe('a step with the pointer path', () => {
  it('names the number of points, with the hint', async () => {
    const wrapper = await row(
      hover([
        { x: 12, y: 40, t: 0 },
        { x: 60, y: 18, t: 48 },
        { x: 96, y: 12, t: 120 },
      ]),
    );
    const path = wrapper.get('[data-testid="step-path"]');
    expect(path.text()).toBe('Pointer path (3 points)');
    expect(path.attributes('title')).toContain('Play and the export move the pointer along it');
  });

  it('says one point in the singular', async () => {
    const wrapper = await row(hover([{ x: 1, y: 2, t: 0 }]));
    expect(wrapper.get('[data-testid="step-path"]').text()).toBe('Pointer path (1 point)');
  });

  it('shows nothing for a step without one', async () => {
    for (const step of [hover(), hover([])]) {
      const wrapper = await row(step);
      expect(wrapper.find('[data-testid="step-path"]').exists()).toBe(false);
      current?.unmount();
      current = null;
    }
  });
});
