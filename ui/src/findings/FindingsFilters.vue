<script setup lang="ts">
// The findings panel's filters (RB-060-01, Nuxt UI since RB-080-88): a checkbox per severity with
// its count, and a select each for the plugin and the view, "all" first. The panel is a window, so
// the selects' popovers are fine (design.md, "A popup in a strip view is native"). reka's select
// takes no empty value, so "all" is `ALL` here and the empty filter (`''`) in the panel.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UCheckbox from '@nuxt/ui/components/Checkbox.vue';
import UFormField from '@nuxt/ui/components/FormField.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import { SEVERITIES } from './findings';
import type { Severity } from './types';

/** The select's "all": no plugin id or view id is `*`. */
const ALL = '*';

const props = defineProps<{
  counts: Record<Severity, number>;
  plugins: string[];
  views: { id: number; name: string }[];
}>();
const severities = defineModel<Severity[]>('severities', { required: true });
const plugin = defineModel<string>('plugin', { required: true });
const view = defineModel<number | ''>('view', { required: true });
const { t } = useI18n();

const pluginItems = computed(() => [
  { label: t('findings.filter.all_plugins'), value: ALL },
  ...props.plugins.map((id) => ({ label: id, value: id })),
]);
const viewItems = computed(() => [
  { label: t('findings.filter.all_views'), value: ALL as string | number },
  ...props.views.map((checked) => ({ label: checked.name, value: checked.id as string | number })),
]);

const pluginChoice = computed({
  get: () => (plugin.value === '' ? ALL : plugin.value),
  set: (value: string) => (plugin.value = value === ALL ? '' : value),
});
const viewChoice = computed({
  get: (): string | number => (view.value === '' ? ALL : view.value),
  set: (value: string | number) => (view.value = typeof value === 'number' ? value : ''),
});

function shows(severity: Severity): boolean {
  return severities.value.includes(severity);
}

function toggle(severity: Severity, on: boolean | 'indeterminate'): void {
  const rest = severities.value.filter((item) => item !== severity);
  severities.value = on === true ? SEVERITIES.filter((item) => item === severity || rest.includes(item)) : rest;
}

const severityClass: Record<Severity, string> = {
  error: 'text-error',
  warning: 'text-warning',
  info: 'text-info',
};
</script>

<template>
  <div class="flex flex-wrap items-end gap-4 text-sm">
    <fieldset class="flex flex-wrap items-center gap-3 pb-1.5">
      <legend class="sr-only">{{ t('findings.filter.severity') }}</legend>
      <UCheckbox
        v-for="severity in SEVERITIES"
        :key="severity"
        :model-value="shows(severity)"
        size="sm"
        :data-testid="`severity-${severity}`"
        @update:model-value="(on) => toggle(severity, on)"
      >
        <template #label>
          <span :class="severityClass[severity]" class="font-medium">{{ t(`findings.severity.${severity}`) }}</span>
          <span class="ml-1 text-muted tabular-nums">{{ counts[severity] }}</span>
        </template>
      </UCheckbox>
    </fieldset>
    <UFormField :label="t('findings.filter.plugin')" size="sm">
      <USelect v-model="pluginChoice" :items="pluginItems" size="sm" class="min-w-40" data-testid="plugin" />
    </UFormField>
    <UFormField v-if="views.length > 0" :label="t('findings.filter.view')" size="sm">
      <USelect v-model="viewChoice" :items="viewItems" size="sm" class="min-w-32" data-testid="view" />
    </UFormField>
  </div>
</template>
