import { afterEach, describe, expect, it } from 'vitest';
import { onPanelEvent } from './events';

const fromHost = (data: unknown, source: MessageEventSource | null = window): void => {
  window.dispatchEvent(new MessageEvent('message', { data, source }));
};

let stop: () => void = () => undefined;

afterEach(() => stop());

describe('the events of a Nuxt UI panel', () => {
  it('hands every event of the panels window to the listener, with its view', () => {
    const heard: unknown[] = [];
    stop = onPanelEvent((message, view) => heard.push([message, view]));
    fromHost({ type: 'event', message: { page: 'https://a.test/' }, view: 2 });
    fromHost({ type: 'event', message: { greeted: 'Ada' } });
    fromHost({ type: 'event', message: { page: 'https://b.test/' }, view: 'x' });
    fromHost({ type: 'event', message: { page: 'https://c.test/' } }, null);
    fromHost({ type: 'result', id: 1, ok: true, value: {} });
    fromHost('event');
    expect(heard).toEqual([
      [{ page: 'https://a.test/' }, 2],
      [{ greeted: 'Ada' }, undefined],
      [{ page: 'https://b.test/' }, undefined],
    ]);
  });

  it('stops listening when asked', () => {
    const heard: unknown[] = [];
    onPanelEvent((message) => heard.push(message))();
    fromHost({ type: 'event', message: { greeted: 'Ada' } });
    expect(heard).toEqual([]);
  });
});
