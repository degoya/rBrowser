import { describe, expect, it } from 'vitest';
import { clock, disconnectMessage, qrPath, secondsLeft, showQrMessage, toggleMessage } from './portal';

describe('the portal panel logic', () => {
  it('draws only the dark modules', () => {
    expect(qrPath(['10', '01'])).toBe('M0 0h1v1h-1zM1 1h1v1h-1z');
    expect(qrPath([])).toBe('');
  });

  it('counts the pairing code down without going negative', () => {
    expect(secondsLeft(10_500, 0)).toBe(11);
    expect(secondsLeft(0, 5_000)).toBe(0);
    expect(clock(125)).toBe('2:05');
    expect(clock(0)).toBe('0:00');
  });

  it('runs the portal commands', () => {
    expect(toggleMessage(false)).toEqual({ type: 'runCommand', id: 'portal.toggle', args: { enabled: false } });
    expect(showQrMessage()).toEqual({ type: 'runCommand', id: 'portal.show_qr' });
    expect(disconnectMessage(3)).toEqual({ type: 'runCommand', id: 'portal.disconnect', args: { device: 3 } });
  });
});
