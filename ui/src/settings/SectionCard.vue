<script setup lang="ts">
// One section of the settings as a card (RB-080-13): one `SectionHeader` (the category as the
// eyebrow, the section as the heading), reset for the section, and its keys in one column divided
// by hairlines. In the search the card names where it lives and opens it there ("Show in ...").
// The grid (`layout`) gets its preview beside it; the device presets list follows in the slot,
// the plugin list stands above the keys in the slot `before`.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UCard from '@nuxt/ui/components/Card.vue';
import type { IpcClient } from '../ipc/client';
import type { JsonValue, PathKind, Scope } from '../ipc/protocol';
import GridPreview from './GridPreview.vue';
import WebStoreInstall from './WebStoreInstall.vue';
import { SETTINGS_ICONS } from './icons';
import { flagOf, SHOW_KEYS_KEY } from './flags';
import { projectRelative } from './paths';
import { type CategoryView, type SectionView, type SettingDef, SHORTCUTS_KEY } from './schema';
import SectionHeader from './SectionHeader.vue';
import SettingField from './SettingField.vue';
import ShortcutsEditor from './shortcuts/ShortcutsEditor.vue';
import type { SettingsStore } from './store';

/** The key the grid preview stands beside. */
const GRID_KEY = 'layout';
/** The switch the Web Store field follows (RB-080-23). */
const WEB_STORE_KEY = 'engines.chromium.extensions.allowWebStore';
const BREAKPOINTS_KEY = 'breakpoints';

const props = defineProps<{
  client: IpcClient;
  store: SettingsStore;
  category: CategoryView;
  section: SectionView;
  hasProject: boolean;
  highlighted: string | null;
  searching: boolean;
}>();
const emit = defineEmits<{ open: [key: string] }>();
const { t } = useI18n();

/** A plugin's page is named by the plugin's name from its catalogue, which the core sends with
 * the schema (RB-080-82); its id where there is none. */
function pluginName(id: string): string {
  const key = `plugin.${id}.name`;
  const name = t(key);
  return name === key ? id : name;
}
const title = computed(() => (props.section.mirror ? pluginName(props.section.titleKey) : t(props.section.titleKey)));
const showKeys = computed(() => flagOf(props.store.values.value, SHOW_KEYS_KEY, false));
const changed = computed(() => props.section.defs.filter((def) => originOf(def) !== 'default'));
const breakpointNames = computed(() => {
  const list = props.store.values.value.get(BREAKPOINTS_KEY)?.value;
  const names = new Map<string, string>();
  for (const item of Array.isArray(list) ? list : []) {
    if (typeof item === 'object' && item !== null && !Array.isArray(item) && typeof item.id === 'string') {
      names.set(item.id, typeof item.label === 'string' && item.label !== '' ? item.label : item.id);
    }
  }
  return names;
});

function valueOf(def: SettingDef): JsonValue {
  return props.store.values.value.get(def.key)?.value ?? def.schema.default ?? null;
}

function originOf(def: SettingDef) {
  return props.store.values.value.get(def.key)?.origin ?? 'default';
}

function onSet(def: SettingDef, value: JsonValue, scope: Scope): void {
  void props.store.set(def.key, value, scope);
}

/** The native dialog for `def`: a path inside the project is stored relative to it when the
 * key may live in the project (RB-080-20). */
function pickerFor(def: SettingDef): (kind: PathKind) => Promise<string | null> {
  return async (kind) => {
    const path = await props.store.pickPath(kind, def.labelKey);
    if (path === null || !def.scopeAllowed.includes('project')) {
      return path;
    }
    return projectRelative(path, props.store.schema.value?.project?.root);
  };
}
</script>

<template>
  <UCard
    as="section"
    variant="soft"
    class="scroll-mt-4 overflow-clip"
    :ui="{ header: 'px-4 py-3 sm:px-4', body: 'px-2 py-1 sm:px-2 sm:py-1' }"
    :data-section="`${category.id}.${section.id}`"
  >
    <template #header>
      <div class="flex flex-wrap items-start justify-between gap-2">
        <SectionHeader :eyebrow="t(category.titleKey)" :title="title" />
        <div class="flex shrink-0 items-center gap-1">
          <UButton
            v-if="searching && section.defs[0]"
            color="neutral"
            variant="soft"
            size="xs"
            :label="t('settings.search.show_in', { category: t(category.titleKey), section: title })"
            data-testid="show-in"
            @click="emit('open', section.defs[0].key)"
          />
          <UButton
            v-if="!section.mirror"
            :icon="SETTINGS_ICONS.reset"
            color="neutral"
            variant="ghost"
            size="xs"
            :label="t('settings.reset_section')"
            :disabled="changed.length === 0"
            data-testid="reset-section"
            @click="store.resetAll(changed)"
          />
        </div>
      </div>
    </template>

    <div class="flex flex-col divide-y divide-default">
      <!-- What a section shows above its keys: the plugin list (RB-080-51). -->
      <slot name="before" />
      <template v-for="def in section.defs" :key="`${section.id}-${def.key}`">
        <div
          v-if="def.key === SHORTCUTS_KEY"
          :id="`field-${def.key}`"
          class="px-2 py-3"
          :data-key="def.key"
          :data-highlighted="highlighted === def.key ? 'true' : undefined"
        >
          <ShortcutsEditor :client="client" :show-keys="showKeys" />
        </div>
        <div v-else class="flex flex-wrap items-start gap-x-4">
          <SettingField
            class="min-w-0 flex-1 basis-80"
            :def="store.withSource(def)"
            :value="valueOf(def)"
            :origin="originOf(def)"
            :issues="store.issues.value[def.key] ?? []"
            :has-project="hasProject"
            :highlighted="highlighted === def.key"
            :pick-path="pickerFor(def)"
            :show-keys="showKeys"
            @set="(value, scope) => onSet(def, value, scope)"
            @reset="store.reset(def)"
          />
          <GridPreview v-if="def.key === GRID_KEY" class="px-2 py-3" :layout="valueOf(def)" :names="breakpointNames" />
          <WebStoreInstall v-if="def.key === WEB_STORE_KEY && valueOf(def) === true" class="basis-full px-2 pb-3" :client="client" />
        </div>
      </template>
      <div v-if="$slots.default" class="px-2 py-3">
        <slot />
      </div>
    </div>
  </UCard>
</template>
