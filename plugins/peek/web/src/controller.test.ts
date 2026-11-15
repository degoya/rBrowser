import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { type Bridge, CONFIG_MAX_AGE, Peek } from './controller';
import { inspection } from './fixtures';

/** A bridge that records every request and answers like the component. */
class FakeBridge implements Bridge {
  readonly view = 1;
  sent: { type: string; data?: unknown }[] = [];
  config: Record<string, unknown> = {};
  listeners: ((message: unknown) => void)[] = [];
  stops: (() => void)[] = [];

  send(message: unknown): Promise<unknown> {
    const request = message as { type: string };
    this.sent.push(request);
    if (request.type === 'config') return Promise.resolve({ config: this.config });
    if (request.type === 'inspect') return Promise.resolve({ inspection: inspection() });
    return Promise.resolve({});
  }

  on(listener: (message: unknown) => void): () => void {
    this.listeners.push(listener);
    return () => (this.listeners = this.listeners.filter((entry) => entry !== listener));
  }

  onStop(listener: () => void): () => void {
    this.stops.push(listener);
    return () => undefined;
  }

  post(message: unknown): void {
    for (const listener of this.listeners) listener(message);
  }

  types(): string[] {
    return this.sent.map((request) => request.type);
  }
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

let bridge: FakeBridge;
let peek: Peek;
let clock = 0;
let button: HTMLButtonElement;

function host(): Element | null {
  return document.querySelector('rbrowser-peek');
}

function move(target: Element, init: PointerEventInit = {}): void {
  target.dispatchEvent(
    new PointerEvent('pointermove', { bubbles: true, composed: true, clientX: 5, clientY: 6, ...init }),
  );
}

/** happy-dom reports AltGraph whenever Alt is down; Chromium only for AltGr. */
function key(type: 'keydown' | 'keyup', init: KeyboardEventInit, altGraph = false): void {
  const event = new KeyboardEvent(type, { bubbles: true, ...init });
  const plain = event.getModifierState.bind(event);
  Object.defineProperty(event, 'getModifierState', {
    value: (name: string) => (name === 'AltGraph' ? altGraph : plain(name)),
  });
  window.dispatchEvent(event);
}

function click(target: Element, init: MouseEventInit = {}): MouseEvent {
  const event = new MouseEvent('click', { bubbles: true, cancelable: true, composed: true, ...init });
  target.dispatchEvent(event);
  return event;
}

beforeEach(async () => {
  button = document.createElement('button');
  button.id = 'send';
  button.textContent = 'Send';
  document.body.append(button);
  bridge = new FakeBridge();
  clock = 0;
  peek = new Peek(window, bridge, () => clock);
  peek.start();
  await flush();
});

afterEach(() => {
  peek.stop();
  document.body.replaceChildren();
});

describe('without the modifier', () => {
  it('Peek takes no event and changes nothing', async () => {
    move(button);
    key('keydown', { key: 'a' });
    const event = click(button);
    await flush();
    expect(event.defaultPrevented).toBe(false);
    expect(host()).toBeNull();
    expect(bridge.types()).toEqual(['config']);
  });

  it('AltGr (Ctrl+Alt) is no Alt', async () => {
    move(button, { altKey: true, ctrlKey: true });
    key('keydown', { key: 'Alt', altKey: true, ctrlKey: true });
    key('keydown', { key: 'Alt', altKey: true }, true);
    await flush();
    expect(host()).toBeNull();
    expect(bridge.types()).toEqual(['config']);
  });
});

describe('with Alt held', () => {
  it('inspects the element under the pointer and shows box and tooltip', async () => {
    move(button, { altKey: true });
    await flush();
    expect(bridge.types()).toEqual(['config', 'inspect']);
    const data = bridge.sent[1]?.data as { tag: string; selector: string };
    expect(data.tag).toBe('button');
    expect(data.selector).toBe('#send');
    expect(host()).not.toBeNull();
    expect(peek.showing).toBe(true);
  });

  it('pressing Alt over a hovered element inspects it, and asks for stale settings', async () => {
    move(button);
    clock = CONFIG_MAX_AGE + 1;
    key('keydown', { key: 'Alt', altKey: true });
    await flush();
    expect(bridge.types()).toEqual(['config', 'config', 'inspect']);
  });

  it('the same element is asked for once', async () => {
    move(button, { altKey: true });
    move(button, { altKey: true });
    await flush();
    expect(bridge.types().filter((type) => type === 'inspect')).toHaveLength(1);
  });

  it('releasing Alt hides everything and clears the other views', async () => {
    move(button, { altKey: true });
    await flush();
    key('keyup', { key: 'Alt' });
    await flush();
    expect(host()).toBeNull();
    expect(bridge.types().at(-1)).toBe('clear');
  });

  it('a click locks the tooltip and is kept from the page; Esc releases it', async () => {
    move(button, { altKey: true });
    await flush();
    const event = click(button, { altKey: true });
    expect(event.defaultPrevented).toBe(true);
    key('keyup', { key: 'Alt' });
    expect(host()).not.toBeNull();
    key('keydown', { key: 'Escape' });
    expect(host()).toBeNull();
  });

  it('a click elsewhere without Alt releases the lock and reaches the page', async () => {
    move(button, { altKey: true });
    await flush();
    click(button, { altKey: true });
    const event = click(document.body);
    expect(event.defaultPrevented).toBe(false);
    expect(host()).toBeNull();
  });
});

describe('settings and commands', () => {
  it('the configured modifier is the one that counts', async () => {
    bridge.config = { modifier: 'Shift' };
    peek.refreshConfig();
    await flush();
    move(button, { altKey: true });
    await flush();
    expect(host()).toBeNull();
    move(button, { shiftKey: true });
    await flush();
    expect(host()).not.toBeNull();
  });

  it('peek.enabled off keeps it asleep; toggle wakes it for this document', async () => {
    bridge.config = { enabled: false };
    peek.refreshConfig();
    await flush();
    move(button, { altKey: true });
    expect(host()).toBeNull();
    bridge.post({ type: 'toggle' });
    expect(peek.enabled).toBe(true);
    move(document.body, { altKey: true });
    await flush();
    expect(host()).not.toBeNull();
    bridge.post({ type: 'toggle' });
    expect(peek.enabled).toBe(false);
    expect(host()).toBeNull();
  });

  it('without sync highlight nothing is cleared in the other views', async () => {
    bridge.config = { syncHighlight: false };
    peek.refreshConfig();
    await flush();
    move(button, { altKey: true });
    await flush();
    key('keyup', { key: 'Alt' });
    expect(bridge.types()).not.toContain('clear');
  });
});

describe('the other views', () => {
  const target = { element: { kind: 'css', path: '#send' }, offset: { x: 0.5, y: 0.5 } };

  it('outline the element another view inspects', () => {
    // happy-dom lays nothing out; the resolution wants a visible box.
    button.getBoundingClientRect = () => new DOMRect(10, 10, 80, 20);
    bridge.post({ type: 'highlight', from: 2, target });
    expect(host()).not.toBeNull();
    bridge.post({ type: 'clear', from: 2 });
    expect(host()).toBeNull();
  });

  it('show nothing - and fail nothing - when the element is not there', () => {
    bridge.post({ type: 'highlight', from: 2, target: { ...target, element: { kind: 'css', path: '#gone' } } });
    bridge.post({ type: 'highlight', from: 2, target: 'garbage' });
    bridge.post({ type: 'highlight', from: 2 });
    expect(host()).toBeNull();
  });

  it('ignore their own posts', () => {
    bridge.post({ type: 'highlight', from: 1, target });
    expect(host()).toBeNull();
  });
});

describe('stopping', () => {
  it('removes the listeners and the overlay', async () => {
    bridge.post({ type: 'highlight', from: 2, target: { element: { kind: 'css', path: '#send' }, offset: { x: 0, y: 0 } } });
    for (const stop of bridge.stops) stop();
    expect(host()).toBeNull();
    move(button, { altKey: true });
    await flush();
    expect(bridge.types()).toEqual(['config']);
  });
});
