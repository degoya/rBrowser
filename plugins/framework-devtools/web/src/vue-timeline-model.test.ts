import { describe, expect, it } from 'vitest';
import { TimelineLog, eventRows, timelineEntry, timelineRows } from './vue-timeline-model';

const payload = {
  layerId: 'pinia:mutations',
  event: {
    time: 1_700_000_000_123,
    title: 'counter',
    subtitle: 'increment',
    logType: 'default',
    groupId: 4,
    data: { store: 'counter', type: 'direct', key: 'count', newValue: 2 },
  },
};

describe('timelineEntry', () => {
  it("takes the kit's event with its layer, defaults for what is missing", () => {
    expect(timelineEntry(payload, 1)).toEqual({
      id: 1,
      layerId: 'pinia:mutations',
      time: 1_700_000_000_123,
      title: 'counter',
      subtitle: 'increment',
      logType: 'default',
      groupId: 4,
      data: payload.event.data,
    });
    expect(timelineEntry({ layerId: 'mouse', event: { time: 5, data: null } }, 2)).toEqual({
      id: 2,
      layerId: 'mouse',
      time: 5,
      title: '',
      subtitle: '',
      logType: 'default',
      groupId: undefined,
      data: null,
    });
  });

  it('refuses anything else', () => {
    expect(timelineEntry(null, 1)).toBeNull();
    expect(timelineEntry({ layerId: 'mouse' }, 1)).toBeNull();
    expect(timelineEntry({ layerId: 5, event: { time: 1 } }, 1)).toBeNull();
    expect(timelineEntry({ layerId: 'mouse', event: { time: 'now' } }, 1)).toBeNull();
    expect(timelineEntry({ layerId: 'mouse', event: { time: 1, title: 3 } }, 1)).toBeNull();
  });
});

describe('TimelineLog', () => {
  it('keeps the newest entries up to its limit, newest first, numbered', () => {
    const log = new TimelineLog(3);
    for (let i = 0; i < 5; i++) {
      expect(log.push({ layerId: 'mouse', event: { time: i, title: `e${i}` } })).toBe(true);
    }
    expect(log.push('junk')).toBe(false);
    expect(log.entries().map((entry) => [entry.id, entry.title])).toEqual([
      [5, 'e4'],
      [4, 'e3'],
      [3, 'e2'],
    ]);
    expect(log.get(4)?.title).toBe('e3');
    expect(log.get(1)).toBeUndefined();
    log.clear();
    expect(log.entries()).toEqual([]);
    expect(log.push(payload)).toBe(true);
    expect(log.entries()[0]?.id).toBe(6);
  });
});

describe('timelineRows', () => {
  it('shows an entry per row with its layer and time as tags, filtered by title, subtitle or layer', () => {
    const log = new TimelineLog();
    log.push(payload);
    log.push({ layerId: 'mouse', event: { time: 1_700_000_000_500, title: 'click' } });
    const all = timelineRows(log.entries(), '');
    expect(all.map((row) => row.label)).toEqual(['click', 'counter: increment']);
    expect(all[1]).toMatchObject({ id: '1', depth: 0, hasChildren: false, expanded: false });
    expect(all[1]?.tags[0]).toBe('pinia:mutations');
    expect(all[1]?.tags[1]).toMatch(/^\d\d:\d\d:\d\d\.123$/);
    expect(timelineRows(log.entries(), 'MOUSE').map((row) => row.label)).toEqual(['click']);
    expect(timelineRows(log.entries(), 'incre').map((row) => row.label)).toEqual(['counter: increment']);
    expect(timelineRows(log.entries(), 'nothing')).toEqual([]);
  });
});

describe('eventRows', () => {
  it("lists the event and its data's keys", () => {
    const entry = timelineEntry(payload, 1);
    expect(entry).not.toBeNull();
    if (!entry) return;
    const rows = eventRows(entry);
    expect(rows.slice(0, 5)).toEqual([
      { section: 'event', key: 'layer', value: 'pinia:mutations' },
      { section: 'event', key: 'time', value: expect.stringMatching(/^\d\d:\d\d:\d\d\.123$/) as string },
      { section: 'event', key: 'title', value: 'counter' },
      { section: 'event', key: 'subtitle', value: 'increment' },
      { section: 'event', key: 'group', value: '4' },
    ]);
    expect(rows.slice(5)).toEqual([
      { section: 'data', key: 'store', value: '"counter"' },
      { section: 'data', key: 'type', value: '"direct"' },
      { section: 'data', key: 'key', value: '"count"' },
      { section: 'data', key: 'newValue', value: '2' },
    ]);
  });

  it('shows a scalar or empty payload as one value', () => {
    const scalar = timelineEntry({ layerId: 'mouse', event: { time: 1, data: 'x' } }, 1);
    expect(scalar && eventRows(scalar).filter((row) => row.section === 'data')).toEqual([
      { section: 'data', key: 'value', value: '"x"' },
    ]);
    const none = timelineEntry({ layerId: 'mouse', event: { time: 1 } }, 1);
    expect(none && eventRows(none).filter((row) => row.section === 'data')).toEqual([]);
  });
});
