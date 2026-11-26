import { describe, expect, it } from 'vitest';

import { parseArgs } from './args.ts';

const known = ['K1', 'K4', 'K10', 'M10'];

describe('parseArgs', () => {
  it('defaults to every criterion from WSL with the standard report', () => {
    const options = parseArgs([], known);
    expect(options).toMatchObject({
      only: null,
      list: false,
      mode: 'wsl',
      keep: false,
      app: null,
      report: 'artifacts/e2e/report.json',
    });
    expect([...options.allowMissing]).toEqual([]);
  });

  it('takes --only as a comma list, case-insensitive, in the order given', () => {
    expect(parseArgs(['--only', 'k10,K4'], known).only).toEqual(['K10', 'K4']);
    expect(parseArgs(['--only=m10'], known).only).toEqual(['M10']);
  });

  it('refuses an unknown criterion and names the known ones', () => {
    expect(() => parseArgs(['--only', 'K99'], known)).toThrow('unknown criterion K99 (known: K1, K4, K10, M10)');
  });

  it('takes --native, --keep, --list, --report, --app, --work', () => {
    const options = parseArgs(
      ['--native', '--keep', '--list', '--report', 'x/r.json', '--app', '/c/app', '--work', 'w'],
      known,
    );
    expect(options).toMatchObject({ mode: 'native', keep: true, list: true, report: 'x/r.json', app: '/c/app', work: 'w' });
  });

  it('allows only firefox and webkit to be missing', () => {
    expect([...parseArgs(['--allow-missing', 'firefox,webkit'], known).allowMissing]).toEqual(['firefox', 'webkit']);
    expect(() => parseArgs(['--allow-missing', 'chrome'], known)).toThrow('--allow-missing takes firefox or webkit');
  });

  it('refuses an unknown option and an option without its value', () => {
    expect(() => parseArgs(['--fast'], known)).toThrow('unknown argument --fast');
    expect(() => parseArgs(['--report'], known)).toThrow('--report needs a value');
  });
});
