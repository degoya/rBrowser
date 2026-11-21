import { describe, expect, it } from 'vitest';
import { clock, cleaned, durationOf, emptyScenario, millis, overLimits, percent, shapeOf, snapshotOf, withShape } from './model';

describe('the load test model', () => {
  it('turns a constant profile into a ramp and back, keeping its length', () => {
    const constant = emptyScenario('a.http').profile;
    expect(shapeOf(constant)).toBe('constant');
    const ramp = withShape(constant, 'ramp');
    expect(shapeOf(ramp)).toBe('ramp');
    expect(durationOf(ramp)).toBe(60);
    expect(ramp.rampUpS).toBe(6);
    const back = withShape(ramp, 'constant');
    expect(back).toMatchObject({ rampUpS: 0, holdS: 60, rampDownS: 0 });
  });

  it('names what passes the ceilings of the settings', () => {
    const scenario = emptyScenario('a.http');
    const limits = { maxUsers: 50, maxRps: 200, maxDurationS: 300 };
    expect(overLimits(scenario, limits)).toEqual([]);
    scenario.profile = { users: 51, rampUpS: 0, holdS: 301, rampDownS: 0, rps: 500 };
    expect(overLimits(scenario, limits)).toEqual(['users', 'rps', 'duration']);
  });

  it('leaves out an empty rate, empty thresholds and no environment', () => {
    const scenario = { ...emptyScenario('a.http'), environment: '' };
    scenario.profile.rps = 0;
    scenario.abort.p95Ms = 0;
    const out = cleaned(scenario);
    expect('rps' in out.profile).toBe(false);
    expect('p95Ms' in out.abort).toBe(false);
    expect('environment' in out).toBe(false);
  });

  it('formats times, shares and the clock', () => {
    expect(millis(4.25)).toBe('4.3 ms');
    expect(millis(250)).toBe('250 ms');
    expect(millis(1250)).toBe('1.25 s');
    expect(millis(null)).toBe('-');
    expect(percent(0.025)).toBe('2.5 %');
    expect(percent(0.5)).toBe('50 %');
    expect(clock(75)).toBe('1:15');
  });

  it('reads a snapshot from a panel event only', () => {
    expect(snapshotOf({ loadTest: { running: true, series: [] } })).not.toBeNull();
    expect(snapshotOf({ send: true })).toBeNull();
    expect(snapshotOf({ loadTest: { running: 'yes', series: [] } })).toBeNull();
    expect(snapshotOf(null)).toBeNull();
  });
});
