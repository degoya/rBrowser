import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { h } from 'vue';
import PanelForm from './PanelForm.vue';

// RB-096-13: a panel's frame is `sandbox="allow-scripts"` without `allow-forms`, and Chromium
// checks that before it fires `submit` - a `<form @submit.prevent>` never ran its handler there
// ("Blocked form submission ... 'allow-forms' permission is not set"). PanelForm is no form: it
// answers Enter in a field itself.
describe('a form in a panel', () => {
  function mounted(disabled = false) {
    return mount(PanelForm, {
      props: { disabled },
      attrs: { id: 'one', 'aria-label': 'Name' },
      slots: {
        default: () => [h('input', { id: 'text' }), h('textarea', { id: 'area' }), h('input', { id: 'check', type: 'checkbox' }), h('button', { id: 'ok', type: 'button' }, 'OK')],
      },
    });
  }

  it('renders no form element, so the sandbox has nothing to block', () => {
    const wrapper = mounted();
    expect(wrapper.find('form').exists()).toBe(false);
    expect(wrapper.attributes('role')).toBe('form');
    expect(wrapper.attributes('id')).toBe('one');
  });

  it('answers Enter in a text field, not in a text area, a checkbox or a button', async () => {
    const wrapper = mounted();
    await wrapper.get('#text').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('submit')).toHaveLength(1);
    await wrapper.get('#area').trigger('keydown', { key: 'Enter' });
    await wrapper.get('#check').trigger('keydown', { key: 'Enter' });
    await wrapper.get('#ok').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('submit')).toHaveLength(1);
  });

  it('answers nothing while disabled or while a word is composed', async () => {
    const off = mounted(true);
    await off.get('#text').trigger('keydown', { key: 'Enter' });
    expect(off.emitted('submit')).toBeUndefined();
    const on = mounted();
    await on.get('#text').trigger('keydown', { key: 'Enter', isComposing: true });
    expect(on.emitted('submit')).toBeUndefined();
  });
});
