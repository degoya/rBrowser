import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import { sampleState } from '../ipc/mockState';
import type { CoreToShell, FlowRunState, FlowSummary, ShellToCore } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import FlowsPage from './FlowsPage.vue';

const FLOWS: FlowSummary[] = [{ name: 'login', description: 'Log in as the test user' }, { name: 'cart' }];

const LAST: FlowRunState = {
  project: true,
  last: {
    version: 1,
    runId: '20261117-100000-d4e5f6',
    flow: 'login',
    file: '.rbrowser/flows/login.ts',
    startedAt: '2026-11-17T10:00:00Z',
    durationMs: 2400,
    passed: false,
    views: [
      {
        viewId: 1,
        label: 'Mobile',
        breakpoint: 'mobile',
        engine: 'chromium',
        status: 'failed',
        durationMs: 2300,
        steps: [{ name: 'log in', status: 'failed', durationMs: 2000 }],
        error: { message: 'expect(locator).toBeVisible() failed', step: 'log in', line: 7 },
        screenshot: 'mobile-1.png',
      },
      {
        viewId: 2,
        label: 'Desktop',
        breakpoint: 'desktop',
        engine: 'chromium',
        status: 'passed',
        durationMs: 1800,
        steps: [],
        screenshot: null,
      },
    ],
  },
};

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

/** The panel's clock in the tests. */
const NOW = Date.UTC(2026, 10, 17, 10, 16, 5);

async function mounted(
  state: FlowRunState = LAST,
  flows: FlowSummary[] = FLOWS,
  pollMs = 1000,
  /** `automation.flows.confirmDelete`; the core answers no settings when absent. */
  confirmDelete?: boolean,
) {
  const listeners = new Set<(message: CoreToShell) => void>();
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };
  const asked: ShellToCore[] = [];
  let answer = state;
  let listed = flows;
  const client = new IpcClient({
    send: async (message) => {
      asked.push(message);
      switch (message.type) {
        case 'listFlows':
          return { type: 'flows', items: listed };
        case 'getSettings':
          if (confirmDelete === undefined) {
            return { type: 'ack' };
          }
          return {
            type: 'settings',
            values: [{ key: 'automation.flows.confirmDelete', value: confirmDelete, origin: 'user' }],
          };
        case 'getFlowRun':
          return { type: 'flowRun', ...answer };
        default:
          return { type: 'ack' };
      }
    },
    subscribe: () => () => undefined,
  });
  const result = await mountWithShell(FlowsPage, {
    mock: { state: sampleState() },
    mount: { props: { client, subscribe, pollMs, now: () => NOW } },
  });
  current = result.wrapper;
  await settle();
  return {
    wrapper: result.wrapper,
    asked,
    push: (message: CoreToShell) => listeners.forEach((listener) => listener(message)),
    answerNext: (next: FlowRunState) => {
      answer = next;
    },
    listNext: (next: FlowSummary[]) => {
      listed = next;
    },
  };
}

const commands = (asked: ShellToCore[]) => asked.filter((message) => message.type === 'runCommand');

describe('the Flows panel (RB-090-60)', () => {
  it('lists the project flows and runs one in every view, in one view, or all of them', async () => {
    const { wrapper, asked } = await mounted();
    const login = wrapper.get('[data-testid="flow-login"]');
    expect(login.text()).toContain('login');
    expect(login.text()).toContain('Log in as the test user');
    await login.get('[data-testid="flow-run"]').trigger('click');
    await settle();
    const view = sampleState().views[0];
    if (view === undefined) {
      throw new Error('the sample has views');
    }
    await wrapper.get(`[data-testid="flow-target"] [role="radio"][value="${view.id}"]`).trigger('click');
    await settle();
    await wrapper.get('[data-testid="flow-cart"] [data-testid="flow-run"]').trigger('click');
    await wrapper.get('[data-testid="flows-run-all"]').trigger('click');
    await settle();
    expect(commands(asked)).toEqual([
      { type: 'runCommand', id: 'flows.run', args: { name: 'login' } },
      { type: 'runCommand', id: 'flows.run', args: { name: 'cart', views: [view.id] } },
      { type: 'runCommand', id: 'flows.run_all' },
    ]);
  });

  it('shows the last result: pass or fail per view with the step, the line and the screenshot', async () => {
    const { wrapper, asked } = await mounted();
    const result = wrapper.get('[data-testid="flows-result"]');
    expect(result.text()).toContain('login');
    expect(wrapper.get('[data-testid="flows-result-status"]').text()).toBe('Failed');
    const mobile = wrapper.get('[data-testid="result-view-1"]');
    expect(mobile.text()).toContain('Mobile');
    expect(mobile.text()).toContain('Failed');
    expect(mobile.text()).toContain('log in');
    expect(mobile.text()).toContain('line 7');
    expect(mobile.text()).toContain('expect(locator).toBeVisible() failed');
    expect(mobile.text()).toContain('mobile-1.png');
    expect(wrapper.get('[data-testid="result-view-2"]').text()).toContain('Passed');
    await wrapper.get('[data-testid="flows-open-results"]').trigger('click');
    await settle();
    expect(commands(asked)).toContainEqual({ type: 'runCommand', id: 'flows.open_results' });
  });

  it('follows a run going on view by view and stops it', async () => {
    const running: FlowRunState = {
      project: true,
      running: { runId: 'r1', flow: 'login', views: [{ view: 1, status: 'running' }], startedAtMs: NOW },
    };
    const { wrapper, asked, answerNext } = await mounted(running, FLOWS, 10);
    expect(wrapper.get('[data-testid="flows-running"]').text()).toContain('login');
    expect(wrapper.get('[data-testid="running-view-1"]').text()).toContain('Running');
    answerNext({
      project: true,
      running: { runId: 'r1', flow: 'login', views: [{ view: 1, status: 'passed' }], startedAtMs: NOW },
    });
    // Asked again while the run goes on, without an event.
    await vi.waitFor(() => expect(wrapper.get('[data-testid="running-view-1"]').text()).toContain('Passed'));
    await wrapper.get('[data-testid="flows-stop"]').trigger('click');
    await settle();
    expect(commands(asked)).toContainEqual({ type: 'runCommand', id: 'flows.cancel' });
  });

  it('shows Stop, every view, the time, the waiting flows and why Run waits while a run goes on (RB-090-71)', async () => {
    const running: FlowRunState = {
      project: true,
      running: {
        runId: 'r1',
        flow: 'login',
        views: [
          { view: 1, status: 'running' },
          { view: 2, status: 'queued' },
          { view: 3, status: 'skipped' },
        ],
        startedAtMs: NOW - 65_000,
        queued: ['cart'],
      },
    };
    const { wrapper, asked } = await mounted(running);
    const stop = wrapper.get('[data-testid="flows-stop"]');
    expect(stop.text()).toBe('Stop');
    expect(wrapper.get('[data-testid="flows-elapsed"]').text()).toContain('1:05');
    expect(wrapper.get('[data-testid="running-view-1"]').text()).toContain('Mobile');
    expect(wrapper.get('[data-testid="running-view-1"]').text()).toContain('Running');
    expect(wrapper.get('[data-testid="running-view-2"]').text()).toContain('Waiting');
    expect(wrapper.get('[data-testid="running-view-3"]').text()).toContain('Skipped');
    expect(wrapper.get('[data-testid="flows-queued"]').text()).toContain('cart');
    const busy = wrapper.get('[data-testid="flows-busy"]').text();
    expect(busy).toContain('login');
    expect(busy).toContain('Stop');
    for (const button of wrapper.findAll('[data-testid="flow-run"]')) {
      expect(button.attributes('disabled')).toBeDefined();
    }
    expect(wrapper.get('[data-testid="flows-run-all"]').attributes('disabled')).toBeDefined();
    await stop.trigger('click');
    await settle();
    expect(commands(asked)).toEqual([{ type: 'runCommand', id: 'flows.cancel' }]);
  });

  it('shows neither Stop nor the reason while no run goes on', async () => {
    const { wrapper } = await mounted();
    expect(wrapper.find('[data-testid="flows-stop"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="flows-busy"]').exists()).toBe(false);
  });

  it('asks again when a run starts or ends', async () => {
    const { wrapper, push, answerNext } = await mounted({ project: true });
    expect(wrapper.find('[data-testid="flows-result"]').exists()).toBe(false);
    answerNext(LAST);
    push({ type: 'flowRunChanged' });
    await settle();
    expect(wrapper.find('[data-testid="flows-result"]').exists()).toBe(true);
  });

  it('says what to do without flows and without a project', async () => {
    const empty = await mounted({ project: true }, []);
    expect(empty.wrapper.find('[data-testid="flows-empty"]').exists()).toBe(true);
    expect(empty.wrapper.get('[data-testid="flows-run-all"]').attributes('disabled')).toBeDefined();
    empty.wrapper.unmount();
    current = null;
    const none = await mounted({ project: false }, []);
    expect(none.wrapper.find('[data-testid="flows-no-project"]').exists()).toBe(true);
  });
});

describe('deleting a flow in the Flows panel (RB-090-91)', () => {
  it('asks in the panel before it deletes, then sends flows.delete and lists again', async () => {
    const { wrapper, asked, push, listNext } = await mounted({ project: true });
    const cart = () => wrapper.get('[data-testid="flow-cart"]');
    await cart().get('[data-testid="delete"]').trigger('click');
    await settle();
    const question = cart().get('[data-testid="delete-confirm"]');
    expect(question.text()).toContain('Delete the flow cart?');
    expect(question.text()).toContain('cannot be undone');
    expect(question.attributes('role')).toBe('alertdialog');
    expect(commands(asked)).toEqual([]);
    // Cancel keeps it.
    await question.get('[data-testid="delete-no"]').trigger('click');
    await settle();
    expect(cart().find('[data-testid="delete-confirm"]').exists()).toBe(false);
    expect(commands(asked)).toEqual([]);
    await cart().get('[data-testid="delete"]').trigger('click');
    await settle();
    await cart().get('[data-testid="delete-yes"]').trigger('click');
    await settle();
    expect(commands(asked)).toEqual([{ type: 'runCommand', id: 'flows.delete', args: { name: 'cart' } }]);
    // The core deleted it and says so: the list is read again.
    listNext([FLOWS[0]!]);
    push({ type: 'flowRunChanged' });
    await settle();
    expect(wrapper.find('[data-testid="flow-cart"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="flow-login"]').exists()).toBe(true);
  });

  it('deletes at once when automation.flows.confirmDelete is off', async () => {
    const { wrapper, asked } = await mounted({ project: true }, FLOWS, 1000, false);
    await wrapper.get('[data-testid="flow-login"] [data-testid="delete"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="delete-confirm"]').exists()).toBe(false);
    expect(commands(asked)).toEqual([{ type: 'runCommand', id: 'flows.delete', args: { name: 'login' } }]);
  });

  it('keeps Delete of the running flow waiting and says why', async () => {
    const running: FlowRunState = {
      project: true,
      running: { runId: 'r1', flow: 'login', views: [{ view: 1, status: 'running' }], startedAtMs: NOW },
    };
    const { wrapper } = await mounted(running);
    const login = wrapper.get('[data-testid="flow-login"] [data-testid="delete"]');
    expect(login.attributes('disabled')).toBeDefined();
    expect(login.attributes('title')).toContain('login is running');
    const cart = wrapper.get('[data-testid="flow-cart"] [data-testid="delete"]');
    expect(cart.attributes('disabled')).toBeUndefined();
    expect(cart.attributes('aria-label')).toBe('Delete cart');
  });

  it('keeps Delete of a flow a Run all still waits to start waiting too', async () => {
    const running: FlowRunState = {
      project: true,
      running: { runId: 'r1', flow: 'login', views: [], startedAtMs: NOW, queued: ['cart'] },
    };
    const { wrapper } = await mounted(running);
    const cart = wrapper.get('[data-testid="flow-cart"] [data-testid="delete"]');
    expect(cart.attributes('disabled')).toBeDefined();
    expect(cart.attributes('title')).toContain('cart is running or waits to run');
  });
});
