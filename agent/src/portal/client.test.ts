import { describe, expect, it, vi } from 'vitest';
import {
  FIRST_PAUSE_MS,
  MAX_FAILURES,
  PortalClient,
  type PortalWindow,
  type SocketLike,
  handle,
  isLocalPath,
  isToDevice,
  socketUrl,
} from './client';

function win(path = '/a'): PortalWindow & { assigned: string[]; timers: Array<[() => void, number]> } {
  const assigned: string[] = [];
  const timers: Array<[() => void, number]> = [];
  return {
    assigned,
    timers,
    location: {
      protocol: 'http:',
      host: '192.168.1.20:4000',
      pathname: path,
      search: '',
      hash: '',
      assign: (url: string | URL) => {
        assigned.push(String(url));
      },
    },
    document,
    setTimeout: (handler: () => void, ms: number) => {
      timers.push([handler, ms]);
      return 0;
    },
  };
}

class FakeSocket implements SocketLike {
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  sent: string[] = [];
  constructor(readonly url: string) {}
  send(data: string): void {
    this.sent.push(data);
  }
  receive(data: unknown): void {
    this.onmessage?.({ data: JSON.stringify(data) } as MessageEvent);
  }
}

describe('the portal client', () => {
  it('connects to the portal it came from', () => {
    expect(socketUrl({ protocol: 'http:', host: '192.168.1.20:4000' })).toBe('ws://192.168.1.20:4000/__rbrowser/ws');
  });

  it('follows only paths on its own origin', () => {
    expect(isLocalPath('/a?b#c')).toBe(true);
    expect(isLocalPath('//evil.example.com/')).toBe(false);
    expect(isLocalPath('http://evil.example.com/')).toBe(false);
    expect(isLocalPath('/\\evil')).toBe(false);
    expect(isToDevice({ kind: 'navigate', path: '//evil' })).toBe(false);
    expect(isToDevice({ kind: 'event', event: { type: 'click' } })).toBe(true);
    expect(isToDevice({ kind: 'eval', code: 'x' })).toBe(false);
    expect(isToDevice(null)).toBe(false);
  });

  it('navigates only when the page differs', () => {
    const here = win('/a');
    expect(handle({ kind: 'navigate', path: '/a' }, here)).toBeNull();
    expect(here.assigned).toEqual([]);
    handle({ kind: 'navigate', path: '/b?x' }, here);
    expect(here.assigned).toEqual(['/b?x']);
  });

  it('says hello, replays and reports each replay', () => {
    const sockets: FakeSocket[] = [];
    const client = new PortalClient(win(), (url) => {
      const socket = new FakeSocket(url);
      sockets.push(socket);
      return socket;
    });
    client.start();
    const socket = sockets[0]!;
    socket.onopen?.(new Event('open'));
    expect(JSON.parse(socket.sent[0]!)).toEqual({ kind: 'hello' });
    socket.receive({ kind: 'event', event: { type: 'hover', target: { element: { kind: 'css', path: 'a' }, offset: { x: 0, y: 0 } } } });
    expect(JSON.parse(socket.sent[1]!)).toEqual({ kind: 'replayed', status: 'unsupported' });
    socket.onmessage?.({ data: 'not json' } as MessageEvent);
    socket.receive({ kind: 'unknown' });
    expect(socket.sent).toHaveLength(2);
  });

  it('reports a missed origin at once while open, and all of them again on a new socket', () => {
    const here = win();
    const sockets: FakeSocket[] = [];
    const client = new PortalClient(here, (url) => {
      const socket = new FakeSocket(url);
      sockets.push(socket);
      return socket;
    });
    client.start();
    client.reportMissed('http://localhost:5173', 'img');
    const first = sockets[0]!;
    expect(first.sent).toEqual([]);
    first.onopen?.(new Event('open'));
    client.reportMissed('http://project.test');
    expect(first.sent.map((text) => JSON.parse(text))).toEqual([
      { kind: 'hello' },
      { kind: 'missed', origin: 'http://localhost:5173', via: 'img' },
      { kind: 'missed', origin: 'http://project.test' },
    ]);
    first.onclose?.({} as CloseEvent);
    client.reportMissed('http://127.0.0.1:9000');
    expect(first.sent).toHaveLength(3);
    here.timers[0]![0]();
    const second = sockets[1]!;
    second.onopen?.(new Event('open'));
    expect(second.sent.map((text) => JSON.parse(text).origin)).toEqual([
      undefined,
      'http://localhost:5173',
      'http://project.test',
      'http://127.0.0.1:9000',
    ]);
  });

  it('retries with a growing pause and gives up after the failures', () => {
    const here = win();
    const open = vi.fn((url: string) => new FakeSocket(url));
    const client = new PortalClient(here, open);
    client.start();
    for (let attempt = 0; attempt < MAX_FAILURES; attempt += 1) {
      const socket = open.mock.results[attempt]!.value as FakeSocket;
      socket.onclose?.({} as CloseEvent);
      const timer = here.timers[attempt];
      if (attempt < MAX_FAILURES - 1) {
        expect(timer?.[1]).toBe(FIRST_PAUSE_MS * 2 ** attempt);
        timer![0]();
      } else {
        expect(timer).toBeUndefined();
      }
    }
    expect(open).toHaveBeenCalledTimes(MAX_FAILURES);
  });

  it('flashes the presenter\'s clicks on a viewer that was told so (RB-096-05)', () => {
    document.body.innerHTML = '<button id="buy">Buy</button>';
    const button = document.getElementById('buy')!;
    button.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, right: 80, bottom: 20, width: 80, height: 20, toJSON: () => ({}) });
    const click = {
      kind: 'event' as const,
      event: { type: 'click' as const, button: 'left' as const, detail: 1, modifiers: 0, target: { element: { kind: 'css' as const, path: '#buy' }, offset: { x: 0.5, y: 0.5 } } },
    };
    expect(isToDevice({ kind: 'present', pointer: true })).toBe(true);
    expect(isToDevice({ kind: 'present', pointer: 'yes' })).toBe(false);
    const viewer = win();
    expect(handle(click, viewer)).toEqual({ kind: 'replayed', status: 'done' });
    expect(document.querySelector('[data-rbrowser-flash]')).toBeNull();
    expect(handle({ kind: 'present', pointer: true }, viewer)).toBeNull();
    expect(handle(click, viewer)).toEqual({ kind: 'replayed', status: 'done' });
    expect(document.querySelector('[data-rbrowser-flash]')).not.toBeNull();
    document.querySelectorAll('[data-rbrowser-flash]').forEach((ring) => ring.remove());
    handle({ kind: 'present', pointer: false }, viewer);
    handle(click, viewer);
    expect(document.querySelector('[data-rbrowser-flash]')).toBeNull();
    document.body.replaceChildren();
  });
});
