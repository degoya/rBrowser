<script setup lang="ts">
// The Licences tab: rBrowser's own licence and the SDK's, then every component it ships with
// its licence - the bundled binaries, the Rust crates, the npm packages (`licences.ts`) - with a
// filter by name or licence and the count of what it shows. A Nuxt UI `UTable` that renders only
// the rows in sight (`virtualize`: some 700 components) under a sticky header, with a `UInput` as
// the filter (RB-080-88).
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import type { TableColumn } from '@nuxt/ui';
import UInput from '@nuxt/ui/components/Input.vue';
import UTable from '@nuxt/ui/components/Table.vue';
import { SETTINGS_ICONS } from '../settings/icons';
import { LICENSE, SDK_LICENSE } from './facts';
import { allComponents, filterComponents, type Component } from './licences';

const { t } = useI18n();
const query = ref('');
const all = allComponents();
const shown = computed(() => filterComponents(all, query.value));

const KIND_KEYS: Record<Component['kind'], string> = {
  bundled: 'settings.about.licenses.kind.bundled',
  rust: 'settings.about.licenses.kind.rust',
  npm: 'settings.about.licenses.kind.npm',
};
const STATUS_KEYS: Record<string, string> = {
  planned: 'settings.about.licenses.status.planned',
  'on-demand': 'settings.about.licenses.status.on_demand',
};

function kindText(component: Component): string {
  const kind = t(KIND_KEYS[component.kind]);
  const status = component.status === undefined ? undefined : STATUS_KEYS[component.status];
  return status === undefined ? kind : `${kind}, ${t(status)}`;
}

const columns = computed<TableColumn<Component>[]>(() => [
  { accessorKey: 'name', header: t('settings.about.licenses.name'), meta: { class: { td: 'text-default' } } },
  { accessorKey: 'version', header: t('settings.about.licenses.version'), meta: { class: { td: 'font-mono' } } },
  { accessorKey: 'license', header: t('settings.about.licenses.license'), meta: { class: { td: 'text-default' } } },
  { id: 'kind', header: t('settings.about.licenses.kind_label'), cell: ({ row }) => kindText(row.original) },
]);

/** One compact row: text-sm plus the cell padding and the border. */
const ROW_HEIGHT = 33;
</script>

<template>
  <div class="flex flex-col gap-4" data-testid="licences-tab">
    <section class="flex flex-col gap-1 text-sm">
      <p>{{ t('settings.about.licenses.own', { license: LICENSE }) }}</p>
      <p>{{ t('settings.about.licenses.sdk', { license: SDK_LICENSE }) }}</p>
      <p class="text-muted">{{ t('settings.about.licenses.help') }}</p>
      <p class="text-muted">{{ t('settings.about.licenses.texts') }}</p>
    </section>
    <div class="flex flex-wrap items-center gap-3">
      <UInput
        v-model="query"
        type="search"
        :icon="SETTINGS_ICONS.search"
        :placeholder="t('settings.about.licenses.filter')"
        :aria-label="t('settings.about.licenses.filter')"
        class="w-72"
        data-testid="licence-filter"
      />
      <span class="text-sm text-muted" data-testid="licence-count">
        {{ t('settings.about.licenses.count', { shown: shown.length, total: all.length }) }}
      </span>
    </div>
    <UTable
      :data="shown"
      :columns="columns"
      :get-row-id="(row: Component) => `${row.kind}:${row.name}@${row.version}`"
      :virtualize="{ estimateSize: ROW_HEIGHT }"
      sticky
      class="max-h-[60vh]"
      :ui="{ th: 'px-2 py-1.5', td: 'px-2 py-1.5' }"
      data-testid="licence-table"
    />
  </div>
</template>
