// The profile choice's logic (RB-090-71): the settings' profiles and the presets taken from forms
// in groups of their own, and the state of a new name for a stored preset.
import { describe, expect, it } from 'vitest';
import { profileGroups, renameState } from './panel/presetChoice';

const t = (key: string): string => key;

describe('the profile choice', () => {
  it('groups no profile, the settings profiles and the stored presets they do not shadow', () => {
    expect(profileGroups(t, ['testuser'], ['Login', 'testuser'], 'NONE')).toEqual([
      [{ label: 'panel.noProfile', value: 'NONE' }],
      [{ type: 'label', label: 'panel.choice.profiles' }, { label: 'testuser', value: 'testuser' }],
      [{ type: 'label', label: 'panel.choice.presets' }, { label: 'Login', value: 'Login' }],
    ]);
  });

  it('leaves out a group with nothing in it', () => {
    expect(profileGroups(t, [], ['Login'], 'NONE')).toEqual([
      [{ label: 'panel.noProfile', value: 'NONE' }],
      [{ type: 'label', label: 'panel.choice.presets' }, { label: 'Login', value: 'Login' }],
    ]);
    expect(profileGroups(t, [], [], 'NONE')).toEqual([[{ label: 'panel.noProfile', value: 'NONE' }]]);
  });

  it('names what a new name for a preset does', () => {
    const settings = ['testuser'];
    const presets = ['Login', 'Checkout'];
    expect(renameState('  ', 'Login', settings, presets)).toBe('empty');
    expect(renameState('x'.repeat(81), 'Login', settings, presets)).toBe('tooLong');
    expect(renameState(' Login ', 'Login', settings, presets)).toBe('same');
    expect(renameState('testuser', 'Login', settings, presets)).toBe('inSettings');
    expect(renameState('Checkout', 'Login', settings, presets)).toBe('taken');
    expect(renameState('Sign-up', 'Login', settings, presets)).toBe('ok');
  });
});
