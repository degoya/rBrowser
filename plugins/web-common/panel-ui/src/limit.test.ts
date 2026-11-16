import { describe, expect, it } from 'vitest';
import { ENVELOPE_BYTES, MAX_BUDGET, MESSAGE_LIMIT_KEY, answerBudget, messageLimit } from './limit';

const MIB = 1024 * 1024;

// RB-090-44: a panel sizes the answers it asks for by the panels window's message limit, the
// setting the host sends with the panel's settings, instead of a fixed 60 000 bytes.
describe('the message limit of a panel', () => {
  it('is the setting the host sent, else the page scripts\' 64 KiB', () => {
    expect(messageLimit({ [MESSAGE_LIMIT_KEY]: 4 * MIB })).toBe(4 * MIB);
    expect(messageLimit({})).toBe(64 * 1024);
    expect(messageLimit({ [MESSAGE_LIMIT_KEY]: 'big' })).toBe(64 * 1024);
    expect(messageLimit({ [MESSAGE_LIMIT_KEY]: 1 })).toBe(64 * 1024);
  });

  it('gives an answer budget that leaves room for the envelope, at most what a tool takes', () => {
    expect(answerBudget({ [MESSAGE_LIMIT_KEY]: 64 * 1024 })).toBe(60_000);
    expect(answerBudget({})).toBe(60_000);
    expect(answerBudget({ [MESSAGE_LIMIT_KEY]: MIB })).toBe(MIB - ENVELOPE_BYTES);
    expect(answerBudget({ [MESSAGE_LIMIT_KEY]: 4 * MIB })).toBe(4 * MIB - ENVELOPE_BYTES);
    expect(answerBudget({ [MESSAGE_LIMIT_KEY]: 16 * MIB })).toBe(MAX_BUDGET);
    expect(answerBudget({ [MESSAGE_LIMIT_KEY]: 16 * MIB }, 500_000)).toBe(500_000);
  });
});
