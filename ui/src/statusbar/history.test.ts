import { describe, expect, it } from 'vitest';
import type { NotificationLevel, StatusMessage } from '../ipc/protocol';
import { copiedText, shown } from './history';

const at = new Date(2026, 10, 12, 9, 5, 7).getTime();
const m = (id: number, level: NotificationLevel): StatusMessage => ({ id, at, level, messageKey: `k.${id}` });
const items = [m(1, 'info'), m(2, 'error'), m(3, 'warning'), m(4, 'error')];

describe('the message history', () => {
  it('lists the newest first, all or one level', () => {
    expect(shown(items, 'all').map((x) => x.id)).toEqual([4, 3, 2, 1]);
    expect(shown(items, 'error').map((x) => x.id)).toEqual([4, 2]);
    expect(shown(items, 'success')).toEqual([]);
    expect(items.map((x) => x.id)).toEqual([1, 2, 3, 4]);
  });

  it('copies what it shows, one line per message', () => {
    const text = copiedText(shown(items, 'error'), (level) => level.toUpperCase(), (x) => `text ${x.id}`);
    expect(text.split('\n')).toHaveLength(2);
    expect(text).toMatch(/^09.05.07\tERROR\ttext 4\n09.05.07\tERROR\ttext 2$/);
  });
});
