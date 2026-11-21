import { describe, expect, it } from 'vitest';
import { nextStep, previousStep, settingsHash, startStep, STEPS } from './steps';

describe('the setup wizard steps', () => {
  it('are the owner decision\'s five with the AI agent step before the end (RB-091-09)', () => {
    expect(STEPS).toEqual(['welcome', 'plugins', 'project', 'basics', 'agent', 'done']);
  });

  it('start at the welcome, or at the project step the hint opens', () => {
    expect(startStep(null)).toBe('welcome');
    expect(startStep('project')).toBe('project');
    expect(startStep('nonsense')).toBe('welcome');
    expect(startStep('agent')).toBe('agent');
  });

  it('step forward and back and stop at the ends', () => {
    expect(nextStep('welcome')).toBe('plugins');
    expect(nextStep('basics')).toBe('agent');
    expect(nextStep('agent')).toBe('done');
    expect(nextStep('done')).toBe('done');
    expect(previousStep('plugins')).toBe('welcome');
    expect(previousStep('welcome')).toBe('welcome');
  });

  it('opens a setting in the same window by its route', () => {
    expect(settingsHash('ui.theme')).toBe('#/settings/ui.theme');
    expect(settingsHash('plugins', 'catalog')).toBe('#/settings/plugins?tab=catalog');
  });
});
