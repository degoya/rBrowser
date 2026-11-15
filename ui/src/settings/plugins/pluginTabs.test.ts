import { describe, expect, it } from 'vitest';
import { parseInstalled } from './installed';
import { countTabs, pluginTab } from './pluginTabs';
import { SAMPLE_INSTALLED } from './sample';

const plugins = parseInstalled(SAMPLE_INSTALLED).plugins;
const byName = (name: string) => {
  const found = plugins.find((plugin) => plugin.id === name || plugin.file === name);
  if (found === undefined) throw new Error(name);
  return found;
};

describe('the plugin tabs (RB-080-80)', () => {
  it('lists a running plugin under Installed, a switched-off or failed one under Disabled', () => {
    expect(pluginTab(byName('seo'), [])).toBe('installed');
    expect(pluginTab(byName('seo'), ['seo'])).toBe('disabled');
    // Off at this start, on again for the next: Installed, the row says the next start loads it.
    expect(pluginTab(byName('snippets'), [])).toBe('installed');
    expect(pluginTab(byName('snippets'), ['snippets'])).toBe('disabled');
    // A refused package, whatever id it claims.
    expect(pluginTab(byName('broken-2.0.0.rbplugin'), [])).toBe('disabled');
    expect(pluginTab({ ...byName('acme'), state: 'error' }, [])).toBe('disabled');
    expect(pluginTab({ ...byName('acme'), state: 'blocked' }, [])).toBe('installed');
  });

  it('counts the plugins of each tab', () => {
    expect(countTabs(plugins, ['snippets'])).toEqual({ installed: 3, disabled: 2 });
    expect(countTabs([], [])).toEqual({ installed: 0, disabled: 0 });
  });
});
