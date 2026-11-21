<script setup lang="ts">
// The settings' navigation: the search, "Only changed" and the categories. Wide, a column on the
// left with the categories as a vertical `UNavigationMenu` (RB-080-88). Narrow (RB-080-96, below
// `NARROW_SETTINGS`), a bar above the content: the categories a `USelect`, the search and the
// switch beside it, wrapping as needed - the content keeps the whole width for its tab row, and
// the way to every category stays on screen. A search lists hits across every category, so the
// select then names none.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UInput from '@nuxt/ui/components/Input.vue';
import UNavigationMenu, { type NavigationMenuItem } from '@nuxt/ui/components/NavigationMenu.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import USwitch from '@nuxt/ui/components/Switch.vue';
import { SETTINGS_ICONS } from './icons';

export interface SettingsCategoryItem {
  label: string;
  value: string;
}

const props = defineProps<{
  /** The categories as navigation entries (wide). */
  items: NavigationMenuItem[];
  /** The categories as choices (narrow). */
  categories: SettingsCategoryItem[];
  /** The open category; `null` while a search lists hits. */
  current: string | null;
  narrow: boolean;
}>();
const emit = defineEmits<{ select: [id: string] }>();
const query = defineModel<string>('query', { required: true });
const onlyChanged = defineModel<boolean>('onlyChanged', { required: true });
const { t } = useI18n();

const chosen = computed(() => props.current ?? undefined);

function choose(value: unknown): void {
  if (typeof value === 'string') emit('select', value);
}
</script>

<template>
  <nav
    v-if="narrow"
    class="flex shrink-0 flex-wrap items-center gap-2 border-b border-default px-3 py-2"
    :aria-label="t('settings.nav.label')"
    data-testid="settings-nav-bar"
  >
    <USelect
      :items="categories"
      :model-value="chosen"
      :placeholder="t('settings.nav.choose')"
      :aria-label="t('settings.nav.label')"
      class="min-w-40 flex-1"
      data-testid="settings-nav-select"
      @update:model-value="choose"
    />
    <UInput
      v-model="query"
      type="search"
      :icon="SETTINGS_ICONS.search"
      :placeholder="t('settings.search.placeholder')"
      :aria-label="t('settings.search.placeholder')"
      class="min-w-40 flex-1"
      data-testid="search"
    />
    <label class="flex items-center gap-2 text-sm">
      <USwitch v-model="onlyChanged" :aria-label="t('settings.search.only_changed')" data-testid="only-changed" />
      {{ t('settings.search.only_changed') }}
    </label>
  </nav>
  <aside v-else class="flex w-64 shrink-0 flex-col gap-3 overflow-y-auto border-r border-default p-3">
    <UInput
      v-model="query"
      type="search"
      :icon="SETTINGS_ICONS.search"
      :placeholder="t('settings.search.placeholder')"
      :aria-label="t('settings.search.placeholder')"
      class="w-full"
      data-testid="search"
    />
    <label class="flex items-center gap-2 text-sm">
      <USwitch v-model="onlyChanged" :aria-label="t('settings.search.only_changed')" data-testid="only-changed" />
      {{ t('settings.search.only_changed') }}
    </label>
    <UNavigationMenu
      :items="items"
      orientation="vertical"
      color="primary"
      :aria-label="t('settings.nav.label')"
      :ui="{ list: 'gap-0.5', link: 'cursor-pointer' }"
      data-testid="settings-nav"
    />
  </aside>
</template>
