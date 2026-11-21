import { describe, expect, it } from 'vitest';
import { boundText, conditionText, nextText, readAnalysis, readEvent, sizeText, switchRows } from './model';

const ANALYSIS = {
  conditions: [
    { name: 'card', query: '(min-width: 500px)', supported: true, bounds: [{ axis: 'inline', op: '>=', value: '500px', px: 500 }] },
    { name: 'card', query: '(max-width: 300px)', supported: true, bounds: [{ axis: 'inline', op: '<=', value: '300px', px: 300 }] },
    { name: '', query: 'style(--x: 1)', supported: false, bounds: [] },
  ],
  containers: [
    {
      path: '#hero',
      selector: 'article#hero',
      names: ['card'],
      type: 'inline-size',
      conditions: [0, 1],
      views: [
        { view: 1, label: 'Phone', breakpoint: 'phone', inline: { size: 280, below: null, above: 300 }, block: { size: 300, below: null, above: null }, matching: [1], resized: false },
        { view: 2, label: 'Desktop', breakpoint: 'desktop', inline: { size: 640, below: 500, above: null }, block: { size: 300, below: null, above: null }, matching: [0], resized: true },
      ],
      switches: [{ from: 1, to: 2, gained: [0], lost: [1] }],
    },
  ],
  views: [
    { view: 1, label: 'Phone', breakpoint: 'phone', width: 375, containers: 1, unreadableSheets: 0, truncated: false },
    { view: 2, label: 'Desktop', breakpoint: 'desktop', width: 1280, containers: 1, unreadableSheets: 2, truncated: false },
    { view: 3, label: 'Firefox', breakpoint: 'desktop', width: 1280, containers: 0, unreadableSheets: 0, truncated: false, error: 'plugins.error.host_failed' },
  ],
  overlay: true,
};

describe('the panel model', () => {
  it('reads an analysis and refuses anything else', () => {
    const analysis = readAnalysis(ANALYSIS);
    expect(analysis?.conditions).toHaveLength(3);
    expect(analysis?.containers[0]?.views).toHaveLength(2);
    expect(analysis?.views[2]?.error).toBe('plugins.error.host_failed');
    expect(analysis?.overlay).toBe(true);
    expect(readAnalysis(null)).toBeNull();
    expect(readAnalysis({ conditions: 'no' })).toBeNull();
    expect(readAnalysis({ conditions: [], containers: [{ path: 3 }], views: [] })?.containers).toEqual([]);
  });

  it('writes a condition with its name, a bound and a size', () => {
    const analysis = readAnalysis(ANALYSIS);
    expect(conditionText(analysis?.conditions[0])).toBe('card (min-width: 500px)');
    expect(conditionText(analysis?.conditions[2])).toBe('style(--x: 1)');
    expect(conditionText(undefined)).toBe('');
    expect(boundText({ axis: 'inline', op: '>=', value: '50rem', px: 800 })).toBe('≥ 50rem (800 px)');
    expect(boundText({ axis: 'block', op: '<', value: '300px', px: 300 })).toBe('< 300px');
    expect(boundText({ axis: 'inline', op: '<=', value: '3furlongs', px: null })).toBe('≤ 3furlongs');
    expect(sizeText(640.5)).toBe('640.5');
    expect(sizeText(null)).toBe('–');
    expect(nextText({ size: 640, below: 500, above: 900 })).toBe('↓ 500  ↑ 900');
    expect(nextText({ size: 640, below: 500, above: null })).toBe('↓ 500');
    expect(nextText({ size: 10, below: null, above: null })).toBe('');
  });

  it('names the breakpoints that switch a container and what changes there', () => {
    const analysis = readAnalysis(ANALYSIS);
    const container = analysis?.containers[0];
    expect(analysis && container ? switchRows(container, analysis) : null).toEqual([
      { from: 'Phone', to: 'Desktop', gained: ['card (min-width: 500px)'], lost: ['card (max-width: 300px)'] },
    ]);
  });

  it('reads the events that ask for a new analysis', () => {
    expect(readEvent({ type: 'refresh' })).toEqual({ refresh: true });
    expect(readEvent({ type: 'resized', path: '#hero', width: 400 })).toEqual({ refresh: true });
    expect(readEvent({ type: 'reset', path: '#hero', width: 640 })).toEqual({ refresh: true });
    expect(readEvent({ type: 'overlay', on: true })).toEqual({ overlay: true });
    expect(readEvent({ type: 'overlay', on: 'x' })).toBeNull();
    expect(readEvent({ host: 'waiting' })).toBeNull();
    expect(readEvent('text')).toBeNull();
  });
});
