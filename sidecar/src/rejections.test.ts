import { EventEmitter } from 'node:events';

import { describe, expect, it } from 'vitest';

import { handleRejections } from './rejections.ts';

function setup(tolerated: boolean) {
  const source = new EventEmitter();
  const lines: string[] = [];
  const exits: number[] = [];
  handleRejections(source, {
    log: (line) => lines.push(line),
    exit: (code) => exits.push(code),
    tolerates: () => tolerated,
  });
  return { source, lines, exits };
}

describe('handleRejections', () => {
  // Review 1: a process-wide swallow hid every real bug and kept a broken sidecar alive.
  it('ends the sidecar on a rejection nobody handled, as Node would, after logging it', () => {
    const { source, lines, exits } = setup(false);
    source.emit('unhandledRejection', new Error('a real bug'));
    expect(exits).toEqual([1]);
    expect(lines[0]).toBe('an unhandled rejection ends the sidecar: a real bug');
  });

  // RB-080-67: Lighthouse's own rejection after its browser was ended under a cancelled run.
  it('only logs the rejection a cancelled Lighthouse run leaves behind', () => {
    const { source, lines, exits } = setup(true);
    source.emit('unhandledRejection', new Error('Protocol error (Runtime.evaluate): Target closed'));
    expect(exits).toEqual([]);
    expect(lines).toEqual(['a cancelled Lighthouse run left a rejection behind: Protocol error (Runtime.evaluate): Target closed']);
  });

  it('cuts a long reason', () => {
    const { source, lines } = setup(false);
    source.emit('unhandledRejection', 'x'.repeat(600));
    expect(lines[0]).toHaveLength('an unhandled rejection ends the sidecar: '.length + 300);
  });
});
