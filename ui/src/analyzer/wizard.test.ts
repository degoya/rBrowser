import { describe, expect, it } from 'vitest';
import { fakeCore, proposal } from './testing';
import { commandFailureKey, createWizard, gridPreview, hintKind, parseUrls, rowsOf } from './wizard';

describe('the wizard logic', () => {
  it('lists every view for a new config and only the new ones as a diff', () => {
    expect(rowsOf(proposal(false)).map((row) => [row.id, row.selected])).toEqual([
      ['mobile', false],
      ['md', true],
      ['desktop', true],
    ]);
    expect(rowsOf(proposal(true)).map((row) => row.id)).toEqual(['md', 'desktop']);
  });

  it('previews a grid of at most four columns', () => {
    const views = Array.from({ length: 6 }, (_, n) => ({
      id: `b${n}`,
      label: `b${n}`,
      width: 100 * (n + 1),
      height: 100,
      isNew: true,
    }));
    const grid = gridPreview(views);
    expect([grid.columns, grid.rows]).toEqual([4, 2]);
    expect(grid.cells[5]).toMatchObject({ id: 'b5', col: 2, row: 2 });
    expect(gridPreview([]).columns).toBe(1);
  });

  it('takes http(s) URLs one per line', () => {
    expect(parseUrls(' http://a.test/x\nftp://no\nhttps://b.test\nhttp://a.test/x ')).toEqual([
      'http://a.test/x',
      'https://b.test',
    ]);
  });

  it('writes nothing until apply, then only the chosen views', async () => {
    const core = fakeCore();
    const wizard = createWizard(core.client);
    await wizard.analyze(['http://site.test/'], 'edges');
    expect(core.sent).toEqual([
      { type: 'analyzeBreakpoints', urls: ['http://site.test/'], mode: 'edges' },
    ]);
    expect(wizard.phase.value).toBe('running');
    core.emit({ type: 'breakpointProposal', ...proposal(true) });
    expect(wizard.phase.value).toBe('ready');
    expect(core.sent.some((message) => message.type === 'applyBreakpoints')).toBe(false);

    const desktop = wizard.rows.value.find((row) => row.id === 'desktop');
    if (desktop) desktop.selected = false;
    await wizard.apply();
    expect(core.sent.at(-1)).toEqual({
      type: 'applyBreakpoints',
      proposalId: 7,
      selected: ['md'],
      replace: false,
    });
    core.emit({
      type: 'notification',
      level: 'info',
      messageKey: 'analyzer.notice.applied',
      args: { count: '1' },
    });
    expect(wizard.phase.value).toBe('applied');
    wizard.stop();
    expect(core.listeners.size).toBe(0);
  });

  it('shows a failure of the core and ignores other notices', async () => {
    const core = fakeCore();
    const wizard = createWizard(core.client);
    await wizard.analyze([]);
    expect(core.sent).toEqual([{ type: 'analyzeBreakpoints', urls: [] }]);
    core.emit({ type: 'notification', level: 'info', messageKey: 'diagnostics.created' });
    expect(wizard.message.value).toBeNull();
    core.emit({ type: 'notification', level: 'error', messageKey: 'analyzer.error.no_url' });
    expect(wizard.phase.value).toBe('failed');
    expect(wizard.message.value?.key).toBe('analyzer.error.no_url');
  });
});

describe('hintKind', () => {
  it('reads the kind of a static-source hint from its key', () => {
    const hint = (key: string) => ({
      path: 'tailwind.config.js',
      key,
      params: { name: 'lg', path: 'tailwind.config.js' },
    });
    expect(hintKind(hint('plugin.breakpoint-analyzer.hints.range'))).toBe('range');
    expect(hintKind(hint('plugin.breakpoint-analyzer.hints.presets'))).toBe('presets');
    expect(hintKind(hint('plugin.breakpoint-analyzer.hints.unknown'))).toBe('dynamic');
  });
});

describe('commandFailureKey', () => {
  it('reads the i18n key of a failed command from the core message', () => {
    expect(commandFailureKey('command failed: project.error.missing')).toBe('project.error.missing');
    expect(commandFailureKey('invalid arguments: `path` is not a recent project')).toBeNull();
    expect(commandFailureKey('command failed: two words')).toBeNull();
  });
});
