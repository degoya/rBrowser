// The preset dialog's logic (RB-090-70): which fields can be taken, the name's state, the
// arguments of `save` and the profile choice with the stored presets.
import { describe, expect, it } from 'vitest';
import { type Capture, captureReason, chosenAtFirst, excludedOf, isCapture, nameState, presetNames, profileChoices, saveArgs, takeableOf } from './panel/capture';

const t = (key: string, params?: Record<string, unknown>): string => (params ? `${key} ${JSON.stringify(params)}` : key);

const CAPTURE: Capture = {
  view: 2, label: 'Desktop', url: 'https://shop.example.com/register', scope: 'focused',
  form: { selector: '#signup', name: 'Sign up' }, name: 'Sign up', truncated: false, available: true,
  presets: ['Login'], profiles: ['testuser'],
  fields: [
    { locator: 'email', label: 'E-mail', kind: 'email', control: 'email', form: 0, selected: true, value: 'linda@example.com' },
    { locator: 'nick', label: 'Nickname', kind: 'text', control: 'text', form: 0, selected: false, value: '' },
    { locator: 'pw', label: 'Password', kind: 'password', control: 'password', form: 0, selected: false, excluded: 'password' },
    { locator: '', label: 'input:text:3', kind: 'text', control: 'text', form: 0, selected: false, value: 'x', excluded: 'no-locator' },
  ],
};

describe('the preset dialog model', () => {
  it('knows a capture from anything else', () => {
    expect(isCapture(CAPTURE)).toBe(true);
    expect(isCapture({ pending: false })).toBe(false);
    expect(isCapture(null)).toBe(false);
  });

  it('splits the fields that can be taken from those that cannot, and starts from the chosen ones', () => {
    expect(takeableOf(CAPTURE).map((field) => field.locator)).toEqual(['email', 'nick']);
    expect(excludedOf(CAPTURE).map((field) => field.excluded)).toEqual(['password', 'no-locator']);
    expect(chosenAtFirst(CAPTURE)).toEqual(['email']);
  });

  it('says what a name does', () => {
    expect(nameState('  ', CAPTURE)).toBe('empty');
    expect(nameState('Sign up', CAPTURE)).toBe('new');
    expect(nameState(' Login ', CAPTURE)).toBe('replaces');
    expect(nameState('testuser', CAPTURE)).toBe('inSettings');
    expect(nameState('x'.repeat(81), CAPTURE)).toBe('tooLong');
  });

  it('builds the arguments of save from the fields chosen, replacing only a stored preset', () => {
    expect(saveArgs(' Sign up ', CAPTURE, ['email', 'nick', 'pw'])).toEqual({ name: 'Sign up', values: { email: 'linda@example.com', nick: '' }, replace: false });
    expect(saveArgs('Login', CAPTURE, ['email'])).toEqual({ name: 'Login', values: { email: 'linda@example.com' }, replace: true });
  });

  it('words every reason a field is not taken, and passes an unknown one through', () => {
    expect(captureReason(t, 'secret')).toBe('panel.capture.reasons.secret');
    expect(captureReason(t, 'odd')).toBe('odd');
  });

  it('reads the stored presets from a list or a post, and offers them after the settings profiles', () => {
    expect(presetNames({ presets: [{ name: 'a', fields: 1 }, { name: 'b' }, { fields: 2 }, 'c'] })).toEqual(['a', 'b']);
    expect(presetNames({ presets: 'no' })).toEqual([]);
    expect(profileChoices(['testuser'], ['testuser', 'Login'])).toEqual([
      { name: 'testuser', preset: false },
      { name: 'Login', preset: true },
    ]);
  });
});
