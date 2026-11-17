import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { nextTick, ref } from 'vue';
import { PANEL_KEY } from '../mount';
import type { Panel } from '../panel';
import CodeEditor from './CodeEditor.vue';
import { highlighter } from './highlight';

// Review of RB-080-84: the coloured backdrop lies exactly under the textarea - the same box (no
// border on either, the same padding), the textarea's scroll bar kept free on the backdrop, and
// its scroll position taken over when the colours come on after the field was scrolled.
const settings = ref<Record<string, unknown>>({});
const panel = { settings } as unknown as Panel;

function editor() {
  return mount(CodeEditor, { props: { lang: 'json', modelValue: '{"a": 1}\n'.repeat(40) }, global: { provide: { [PANEL_KEY as symbol]: panel } }, attachTo: document.body });
}

afterEach(() => {
  settings.value = {};
  highlighter().configure({});
});

describe('the code editor', () => {
  it("gives the backdrop the textarea's box", () => {
    const wrapper = editor();
    const back = wrapper.find('pre').element as HTMLElement;
    const area = wrapper.find('textarea').element as HTMLTextAreaElement;
    const box = (element: HTMLElement) => element.className.split(/\s+/).filter((name) => /^(border|p[xytrbl]?-|leading-|text-xs|font-mono|whitespace-|break-)/.test(name)).sort();
    expect(box(back)).toEqual(box(area));
    expect(back.className.split(/\s+/)).not.toContain('border');
    wrapper.unmount();
  });

  it('keeps the scroll bar of the textarea free on the backdrop', async () => {
    const wrapper = editor();
    const area = wrapper.find('textarea').element as HTMLTextAreaElement;
    Object.defineProperty(area, 'offsetWidth', { configurable: true, value: 300 });
    Object.defineProperty(area, 'clientWidth', { configurable: true, value: 286 });
    await wrapper.find('textarea').trigger('input');
    await nextTick();
    expect((wrapper.find('pre').element as HTMLElement).style.paddingRight).toBe('calc(0.625rem + 14px)');
    wrapper.unmount();
  });

  it('takes over the scroll position when the colours come on', async () => {
    settings.value = { 'ui.syntaxHighlight': false };
    const wrapper = editor();
    await nextTick();
    expect(wrapper.find('pre').exists()).toBe(false);
    const area = wrapper.find('textarea').element as HTMLTextAreaElement;
    area.scrollTop = 120;
    settings.value = {};
    await nextTick();
    await nextTick();
    expect((wrapper.find('pre').element as HTMLElement).scrollTop).toBe(120);
    wrapper.unmount();
  });
});
