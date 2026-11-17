import { describe, expect, it } from 'vitest';
import { format } from '../../../../web-common/panel-ui/src/format';
import type { Text } from './model';
import { asDifferences, diffLines } from './tab-diffs';

const strings: Record<string, string> = {
  'plugin.overlays.panel.tab_only': '{selector}: only in {views}',
  'plugin.overlays.panel.tab_moved': '{selector}: {positions}',
  'plugin.overlays.panel.tab_position': 'view {view} #{index}',
  'plugin.overlays.panel.tab_same': 'Same order everywhere.',
  'plugin.overlays.panel.tab_more': 'and {count} more',
  'plugin.overlays.panel.tab_single': 'Compare needs two views.',
};
const t: Text = (key, params = {}) => format(strings[`plugin.overlays.${key}`] ?? key, params);

/** `value` as differences, failing the test when it is not their shape. */
function differencesOf(value: unknown) {
  const differences = asDifferences(value);
  if (!differences) throw new Error('not the shape of differences');
  return differences;
}

describe('the tab order differences in the panel', () => {
  it('lists elements only some views reach and stops at other places', () => {
    const differences = differencesOf({
      views: [1, 2],
      onlyIn: [{ selector: 'nav a', path: 'x', views: [1], missing: [2] }],
      onlyInCount: 3,
      moved: [{ selector: 'button.buy', path: 'y', positions: [{ view: 1, index: 3 }, { view: 2, index: 5 }] }],
      movedCount: 1,
    });
    expect(diffLines(t, differences)).toEqual(['nav a: only in 1', 'and 2 more', 'button.buy: view 1 #3, view 2 #5']);
  });

  it('says when every view agrees, and when there is nothing to compare', () => {
    expect(diffLines(t, differencesOf({ views: [1, 2], onlyIn: [], onlyInCount: 0, moved: [], movedCount: 0 }))).toEqual(['Same order everywhere.']);
    expect(diffLines(t, differencesOf({ views: [1], onlyIn: [], moved: [] }))).toEqual(['Compare needs two views.']);
  });

  it('takes nothing that is not the component shape', () => {
    expect(asDifferences(null)).toBeUndefined();
    expect(asDifferences({ views: 'x' })).toBeUndefined();
    const odd = asDifferences({ views: [1, 2], onlyIn: [{ selector: 3 }], moved: [{ selector: 'a', positions: 'x' }] });
    expect(odd?.onlyIn).toEqual([]);
    expect(odd?.moved).toEqual([]);
  });
});
