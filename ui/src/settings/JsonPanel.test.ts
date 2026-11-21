// The JSON view with syntax colours (RB-080-42): one span per token with the kind's classes,
// the text as before, Copy plain JSON, and `settings.jsonHighlight` off a plain
// `<pre>` - live, from the values the panel shows.

import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { createShellI18n } from '../i18n';
import type { SettingValue } from '../ipc/protocol';
import JsonPanel from './JsonPanel.vue';
import { scopeText } from './jsonView';
import { unnamedFocusables } from '../test/keyboard';

const values: SettingValue[] = [
  { key: 'sync.scroll', value: false, origin: 'local' },
  { key: 'sync.delayMs', value: 2.5e3, origin: 'project' },
  { key: 'ui.theme', value: 'dark "mode" \\ é', origin: 'user' },
  { key: 'logs.file', value: null, origin: 'default' },
];

let current: VueWrapper | null = null;

function mountPanel(list: SettingValue[]): VueWrapper {
  current = mount(JsonPanel, {
    props: { values: list },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  return current;
}

afterEach(() => {
  current?.unmount();
  current = null;
});

const pre = (wrapper: VueWrapper) => wrapper.get('[data-testid="json-text"]');

describe('the JSON panel', () => {
  it('colours every kind of token with its classes and keeps the text whole', () => {
    const wrapper = mountPanel(values);
    const text = pre(wrapper);
    expect(text.element.textContent).toBe(scopeText(values, 'effective'));
    const classesOf = (kind: string) =>
      text.findAll(`.json-${kind}`).map((span) => span.classes().sort().join(' '));
    expect(text.findAll('.json-key').map((span) => span.text())).toEqual([
      '"sync"',
      '"scroll"',
      '"delayMs"',
      '"ui"',
      '"theme"',
      '"logs"',
      '"file"',
    ]);
    expect(classesOf('key')[0]).toContain('text-primary');
    expect(text.findAll('.json-string').map((span) => span.text())).toEqual([
      '"dark \\"mode\\" \\\\ é"',
    ]);
    expect(classesOf('string')[0]).toContain('text-success');
    expect(text.findAll('.json-number').map((span) => span.text())).toEqual(['2500']);
    expect(classesOf('number')[0]).toContain('text-warning');
    expect(text.findAll('.json-boolean').map((span) => span.text())).toEqual(['false']);
    expect(classesOf('boolean')[0]).toContain('text-info');
    expect(text.findAll('.json-null').map((span) => span.text())).toEqual(['null']);
    expect(classesOf('null')[0]).toContain('text-muted');
    expect(text.findAll('.json-punct').length).toBeGreaterThan(5);
    expect(classesOf('punct')[0]).toContain('text-dimmed');
    expect(text.findAll('.json-space').length).toBe(0);
  });

  it('copies the plain JSON of the scope, without markup', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    const wrapper = mountPanel(values);
    await wrapper.get('[data-scope="user"]').trigger('click');
    const buttons = wrapper.findAll('button');
    const copy = buttons.find((button) => button.text() === 'Copy');
    expect(copy).toBeDefined();
    await copy?.trigger('click');
    expect(writeText).toHaveBeenCalledWith(scopeText(values, 'user'));
    expect(writeText.mock.calls[0]?.[0]).not.toContain('<');
  });

  it('shows one colour when settings.jsonHighlight is off, and colours again when it is on', async () => {
    const off: SettingValue[] = [
      ...values,
      { key: 'settings.jsonHighlight', value: false, origin: 'user' },
    ];
    const wrapper = mountPanel(off);
    expect(pre(wrapper).findAll('span').length).toBe(0);
    expect(pre(wrapper).element.textContent).toBe(scopeText(off, 'effective'));
    await wrapper.setProps({
      values: [...values, { key: 'settings.jsonHighlight', value: true, origin: 'default' }],
    });
    await nextTick();
    expect(pre(wrapper).findAll('.json-key').length).toBeGreaterThan(0);
  });

  // RB-090-08: the scrolling text is a Tab stop, so it is a named region, not a bare <pre>.
  it('makes its scrolling text a named region', () => {
    const wrapper = mountPanel(values);
    expect(pre(wrapper).attributes('role')).toBe('region');
    expect(pre(wrapper).attributes('aria-label')).toBe('Settings as JSON');
    expect(unnamedFocusables(wrapper.element)).toEqual([]);
  });
});
