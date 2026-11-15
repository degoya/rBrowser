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
});
