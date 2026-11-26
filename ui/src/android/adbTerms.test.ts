import { describe, expect, it } from 'vitest';
import { acceptance, adbTermsOf } from './adbTerms';

const ARGS = {
  version: '37.0.1',
  size: '8.0',
  license: 'LicenseRef-Android-SDK-License',
  from: 'dl.google.com',
  terms: 'https://developer.android.com/tools/releases/platform-tools',
};

describe('the terms of the adb download (RB-099-13)', () => {
  it('reads what the core sends with its question', () => {
    expect(adbTermsOf(ARGS)).toEqual(ARGS);
  });

  it('takes nothing that is not the question: no arguments, a missing or odd field, a terms page that is no web address', () => {
    expect(adbTermsOf(undefined)).toBeNull();
    expect(adbTermsOf(null)).toBeNull();
    expect(adbTermsOf('terms')).toBeNull();
    expect(adbTermsOf([ARGS])).toBeNull();
    const { version: _version, ...noVersion } = ARGS;
    expect(adbTermsOf(noVersion)).toBeNull();
    expect(adbTermsOf({ ...ARGS, size: 8 })).toBeNull();
    expect(adbTermsOf({ ...ARGS, terms: 'javascript:alert(1)' })).toBeNull();
  });

  it('accepts with the one argument the core downloads on', () => {
    expect(acceptance()).toEqual({ type: 'runCommand', id: 'tools.adb.install', args: { accepted: true } });
  });
});
