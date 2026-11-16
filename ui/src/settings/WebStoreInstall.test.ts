import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import UInput from '@nuxt/ui/components/Input.vue';
import { mountPage, settle } from './test/mountPage';
import WebStoreInstall from './WebStoreInstall.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const ID = 'nhdogjmejiglipccpnnnanhbledajbpd';

async function mounted() {
  const result = await mountPage(WebStoreInstall);
  current = result.wrapper;
  return result;
}

describe('the Web Store field', () => {
  it('runs the install command with the reference and clears the field', async () => {
    const { wrapper, core } = await mounted();
    await wrapper.get('[data-testid="web-store-ref"]').setValue(` https://chromewebstore.google.com/detail/vue/${ID} `);
    await wrapper.get('[data-testid="web-store-install"]').trigger('submit');
    await settle();
    expect(core.sent.filter((m) => m.type === 'runCommand')).toEqual([
      {
        type: 'runCommand',
        id: 'extensions.install_web_store',
        args: { ref: `https://chromewebstore.google.com/detail/vue/${ID}` },
      },
    ]);
    expect(wrapper.get<HTMLInputElement>('[data-testid="web-store-ref"]').element.value).toBe('');
    expect(wrapper.get('[data-testid="web-store-hint"]').text()).toContain('notice follows');
  });

  it('sends nothing for a reference that is no id or store URL', async () => {
    const { wrapper, core } = await mounted();
    await wrapper.get('[data-testid="web-store-ref"]').setValue('https://evil.test/x');
    await wrapper.get('[data-testid="web-store-install"]').trigger('submit');
    await settle();
    expect(core.sent.filter((m) => m.type === 'runCommand')).toEqual([]);
    expect(wrapper.get('[data-testid="web-store-ref"]').attributes('aria-invalid')).toBe('true');
    expect(wrapper.get('[data-testid="web-store-hint"]').text()).toContain('32 letters');
    // RB-080-88: a Nuxt UI `UInput`, red while the reference is refused, plain again on typing.
    const color = () => ((wrapper.findComponent(UInput) as unknown as VueWrapper).props() as Record<string, unknown>).color;
    expect(color()).toBe('error');
    await wrapper.get('[data-testid="web-store-ref"]').setValue(ID);
    expect(color()).toBe('primary');
    expect(wrapper.get('[data-testid="web-store-ref"]').attributes('aria-invalid')).toBeUndefined();
  });
});
