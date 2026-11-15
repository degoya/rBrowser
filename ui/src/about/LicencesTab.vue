<script setup lang="ts">
// The Licences tab: rBrowser's own licence and the SDK's, then every component it ships with
// its licence - the bundled binaries, the Rust crates, the npm packages (`licences.ts`) - with a
// filter by name or licence and the count of what it shows.
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
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
      <input
        v-model="query"
        type="search"
        class="h-8 w-72 rounded-md bg-default px-2 text-sm text-default ring ring-inset ring-accented focus-visible:outline-2 focus-visible:outline-primary"
        :placeholder="t('settings.about.licenses.filter')"
        :aria-label="t('settings.about.licenses.filter')"
        data-testid="licence-filter"
      />
      <span class="text-sm text-muted" data-testid="licence-count">
        {{ t('settings.about.licenses.count', { shown: shown.length, total: all.length }) }}
      </span>
    </div>
    <div class="overflow-x-auto">
      <table class="w-full text-left text-sm">
        <thead class="text-muted">
          <tr>
            <th class="py-1 pr-4 font-medium">{{ t('settings.about.licenses.name') }}</th>
            <th class="py-1 pr-4 font-medium">{{ t('settings.about.licenses.version') }}</th>
            <th class="py-1 pr-4 font-medium">{{ t('settings.about.licenses.license') }}</th>
            <th class="py-1 font-medium">{{ t('settings.about.licenses.kind_label') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="component in shown"
            :key="`${component.kind}:${component.name}@${component.version}`"
            class="border-t border-default"
            data-testid="licence-row"
          >
            <td class="py-1 pr-4">{{ component.name }}</td>
            <td class="py-1 pr-4 font-mono">{{ component.version }}</td>
            <td class="py-1 pr-4">{{ component.license }}</td>
            <td class="py-1 text-muted">{{ kindText(component) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
