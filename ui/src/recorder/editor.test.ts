// RB-092-01: the pause an added step starts with, and "Pick in view" through the core.
import { describe, expect, it } from 'vitest';
import type { JsonValue, PickedElement } from '../ipc/protocol';
import { insertedPauseOf, pickedOnce } from './editor';

const ELEMENT: PickedElement = {
  view: 2,
  locator: { selectors: [{ kind: 'css', path: 'aside > button' }] },
  text: 'Filter',
  onlyIn: ['tablet'],
};

function channel(fails = false, sends = true) {
  let listener: ((picked: PickedElement) => void) | null = null;
  const ran: { id: string; args?: JsonValue }[] = [];
  return {
    ran,
    channel: {
      onPicked(heard: (picked: PickedElement) => void) {
        listener = heard;
        return () => {
          listener = null;
        };
      },
      async run(id: string, args?: JsonValue) {
        ran.push({ id, args });
        if (fails) {
          throw new Error('the pick timed out');
        }
        if (sends) {
          listener?.(ELEMENT);
        }
      },
    },
    listening: () => listener !== null,
  };
}

describe('the pause of an added step', () => {
  it('is the setting in whole ms up to 10 s, else half a second', () => {
    expect(insertedPauseOf(750)).toBe(750);
    expect(insertedPauseOf(25_000)).toBe(10_000);
    expect(insertedPauseOf(undefined)).toBe(500);
    expect(insertedPauseOf('fast')).toBe(500);
  });
});

describe('Pick in view', () => {
  it('runs the command for the recording and answers the element it sent', async () => {
    const fake = channel();
    const failures: unknown[] = [];
    await expect(pickedOnce(fake.channel, 'search', (caught) => failures.push(caught))).resolves.toEqual(ELEMENT);
    expect(fake.ran).toEqual([{ id: 'recorder.pick_element', args: { recordingId: 'search' } }]);
    expect(fake.listening()).toBe(false);
    expect(failures).toEqual([]);
  });

  it('answers nothing without a recording, without an element, or when the pick failed', async () => {
    const none = channel();
    await expect(pickedOnce(none.channel, null, () => undefined)).resolves.toBeNull();
    expect(none.ran).toEqual([]);
    await expect(pickedOnce(channel(false, false).channel, 'search', () => undefined)).resolves.toBeNull();
    const failures: unknown[] = [];
    const broken = channel(true);
    await expect(pickedOnce(broken.channel, 'search', (caught) => failures.push(caught))).resolves.toBeNull();
    expect(failures).toHaveLength(1);
    expect(broken.listening()).toBe(false);
  });
});
