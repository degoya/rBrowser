import { describe, expect, it } from 'vitest';
import { fromSettings, hostReason, options, shownOptions, stepped, ZOOMS } from './panel/controls';
import { fileItems, shortName } from './panel/files';

// RB-080-63, RB-080-86: the options of the chosen tool, read within their ranges; the settings'
// starting values; the zoom steps; a host failure in the plugin's words; the image list grouped.
describe('the controls', () => {
  it('show the options of the chosen tool only, the block size not for a black bar', () => {
    expect(shownOptions('rect', 'pixelate')).toEqual(['color', 'width', 'fill']);
    expect(shownOptions('text', 'pixelate')).toEqual(['color', 'size']);
    expect(shownOptions('marker', 'pixelate')).toEqual(['color', 'size']);
    expect(shownOptions('highlight', 'pixelate')).toEqual(['color']);
    expect(shownOptions('redact', 'blur')).toEqual(['mode', 'block']);
    expect(shownOptions('redact', 'black')).toEqual(['mode']);
    expect(shownOptions('crop', 'pixelate')).toEqual(['crop']);
  });

  it('read the drawing options within their ranges', () => {
    expect(options({ color: '#00FF00', width: 6.4, size: 1000, mode: 'blur', block: 2, fill: true })).toEqual({
      color: '#00ff00',
      width: 6,
      size: 256,
      mode: 'blur',
      block: 8,
      fill: true,
    });
    expect(options({ color: 'red', width: 'x', size: Number.NaN, mode: 'other', block: undefined, fill: false })).toEqual({
      color: '#e53935',
      width: 4,
      size: 20,
      mode: 'pixelate',
      block: 12,
      fill: false,
    });
  });

  it('take the plugin settings as starting values', () => {
    expect(fromSettings({ 'screenshot-editor.color': '#00FF00', 'screenshot-editor.strokeWidth': 6, 'screenshot-editor.fontSize': '24', 'screenshot-editor.redactBlock': null })).toEqual({
      color: '#00ff00',
      width: 6,
      size: 24,
    });
    expect(fromSettings({ 'screenshot-editor.color': 'blue', 'ui.theme': 'dark' })).toEqual({});
  });

  it('step the zoom through its steps and stop at the ends', () => {
    expect(stepped(1, 1)).toBe(1.5);
    expect(stepped(1, -1)).toBe(0.75);
    expect(stepped(0.8, 1)).toBe(1);
    expect(stepped(0.8, -1)).toBe(0.75);
    expect(stepped(4, 1)).toBe(4);
    expect(stepped(ZOOMS[0] ?? 0, -1)).toBe(ZOOMS[0]);
    expect(stepped(9, -1)).toBe(3);
  });

  it('say a host failure in the plugin words where it has them', () => {
    const own = (key: string) => (key === 'panel.host_error.busy' ? 'Busy, try again.' : undefined);
    expect(hostReason('plugins.error.busy', own)).toBe('Busy, try again.');
    expect(hostReason('plugins.error.unheard_of', own)).toBeUndefined();
    expect(hostReason('plugin.screenshot-editor.errors.bad_args', own)).toBeUndefined();
  });
});

describe('the image list', () => {
  it('groups the paths by folder in their order, each file by its name', () => {
    const shot = '.rbrowser/out/screenshots/run-1/mobile.png';
    expect(shortName(shot)).toBe('screenshots/run-1/mobile.png');
    expect(
      fileItems([shot, '.rbrowser/out/screenshots/run-1/desktop.png', '.rbrowser/out/checks/run/diff.png', '.rbrowser/out/top.png']),
    ).toEqual([
      [
        { type: 'label', label: 'screenshots/run-1' },
        { label: 'mobile.png', value: shot },
        { label: 'desktop.png', value: '.rbrowser/out/screenshots/run-1/desktop.png' },
      ],
      [
        { type: 'label', label: 'checks/run' },
        { label: 'diff.png', value: '.rbrowser/out/checks/run/diff.png' },
      ],
      [{ label: 'top.png', value: '.rbrowser/out/top.png' }],
    ]);
    expect(fileItems([])).toEqual([]);
  });
});
