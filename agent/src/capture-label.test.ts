// A click on the label of a checkbox or radio (RB-090-93): the click names the control, and the
// engine's own click on the control that follows in the same task is not sent - the label's click
// does it again in every view, and the recorder keeps the label's click as the step.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Agent } from './agent';
import type { AgentConfig, AgentMessage, SyncEvent } from './protocol';
import { el, giveBoxes, loadPage, setViewport } from './test-page';

const ACTIVE: AgentConfig = {
  active: true,
  capture: {
    click: true, hover: true, scroll: true, keyboard: true, input: true, forms: true, focus: true,
  },
  hoverThrottleMs: 100,
};

const PAGE = `<!doctype html><html><head></head><body>
<header>
  <input type="checkbox" id="nav-toggle" class="sr-only">
  <label for="nav-toggle"><span class="bar"></span><span class="bar"></span><span class="sr-only">Menu</span></label>
</header>
<form id="order">
  <label id="small"><input type="radio" name="size" id="size-s" value="s"> Small</label>
  <label for="email" id="email-label">E-mail</label><input id="email" type="email">
</form>
</body></html>`;

let sent: AgentMessage[];
let agent: Agent;

function events(): SyncEvent[] {
  return sent.flatMap((message) => (message.kind === 'event' ? [message.event] : []));
}

function clicks(): SyncEvent[] {
  return events().filter((event) => event.type === 'click');
}

const mouse = { bubbles: true, composed: true, clientX: 35, clientY: 30, detail: 1 };

/** What the engine does for a click on `inner` of a label: the label's click, then - unless the
 *  DOM implementation does it by itself - the click on the control, in the same task. */
function clickLabel(inner: Element, control: HTMLInputElement): void {
  let reached = false;
  const seen = () => {
    reached = true;
  };
  control.addEventListener('click', seen);
  inner.dispatchEvent(new MouseEvent('click', mouse));
  control.removeEventListener('click', seen);
  if (!reached) control.dispatchEvent(new MouseEvent('click', mouse));
}

beforeEach(() => {
  vi.useFakeTimers();
  setViewport(768);
  loadPage(PAGE);
  giveBoxes();
  sent = [];
  agent = new Agent({
    win: window,
    send: (payload) => sent.push(JSON.parse(payload) as AgentMessage),
    config: ACTIVE,
    trusted: () => true,
  });
  agent.start();
});

afterEach(() => {
  agent.stop();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('a click on a label of a checkbox or radio', () => {
  it('names the control and sends no click of its own for the control', () => {
    const toggle = el<HTMLInputElement>('#nav-toggle');
    clickLabel(el('label[for="nav-toggle"] .bar'), toggle);
    expect(clicks()).toHaveLength(1);
    expect(clicks()[0]).toMatchObject({
      type: 'click',
      target: { element: { kind: 'text', tag: 'label', text: 'Menu' } },
      control: '#nav-toggle',
    });
  });

  it('names the control a label wraps', () => {
    const radio = el<HTMLInputElement>('#size-s');
    clickLabel(el('#small'), radio);
    expect(clicks()).toHaveLength(1);
    expect(clicks()[0]).toMatchObject({ control: '#size-s' });
  });

  it('lets a later click on the control through', () => {
    const toggle = el<HTMLInputElement>('#nav-toggle');
    clickLabel(el('label[for="nav-toggle"] .bar'), toggle);
    vi.advanceTimersByTime(1);
    toggle.dispatchEvent(new MouseEvent('click', mouse));
    expect(clicks()).toHaveLength(2);
    expect(clicks()[1]).not.toHaveProperty('control');
  });

  it('names nothing for a click on the checkbox itself', () => {
    el('#nav-toggle').dispatchEvent(new MouseEvent('click', mouse));
    expect(clicks()).toHaveLength(1);
    expect(clicks()[0]).not.toHaveProperty('control');
  });

  it('names nothing for the label of a text field', () => {
    el('#email-label').dispatchEvent(new MouseEvent('click', mouse));
    expect(clicks()[0]).not.toHaveProperty('control');
  });
});
