<script setup lang="ts">
// The findings panel's Export (RB-092-05): a split button of Nuxt UI. The button exports the last
// run in the format of `checks.export.defaultFormat`; the arrow beside it offers every format in a
// `UDropdownMenu` (the panels page's own popup, as the dock bar's menu). A secondary action, so
// both halves are icons with a name and a tooltip naming the command and its shortcut. The panel
// sends the request (`exporting.ts`); the core names the file in the status bar.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UDropdownMenu, { type DropdownMenuItem } from '@nuxt/ui/components/DropdownMenu.vue';
import UFieldGroup from '@nuxt/ui/components/FieldGroup.vue';
import { EXPORT_FORMATS, type ExportFormat } from './exporting';
import { FINDINGS_ICONS } from './icons';

const props = withDefaults(defineProps<{ disabled?: boolean; shortcut?: string | null }>(), {
  disabled: false,
  shortcut: null,
});
const emit = defineEmits<{ export: [format: ExportFormat | null] }>();
const { t } = useI18n();

const label = computed(() => t('findings.export.run'));
const tooltip = computed(() =>
  props.shortcut === null ? label.value : t('shell.menu.button.with_shortcut', { label: label.value, shortcut: props.shortcut }),
);
const items = computed<DropdownMenuItem[]>(() =>
  EXPORT_FORMATS.map((format) => ({
    label: t(`findings.export.format.${format}`),
    onSelect: () => emit('export', format),
  })),
);
</script>

<template>
  <UFieldGroup size="sm" data-testid="export">
    <UButton
      :icon="FINDINGS_ICONS.export"
      color="neutral"
      variant="ghost"
      square
      :disabled="disabled"
      :aria-label="label"
      :title="tooltip"
      data-testid="export-run"
      @click="emit('export', null)"
    />
    <UDropdownMenu :items="items" :disabled="disabled" :content="{ align: 'end', collisionPadding: 4 }" :modal="false">
      <UButton
        :icon="FINDINGS_ICONS.exportMenu"
        color="neutral"
        variant="ghost"
        square
        :disabled="disabled"
        :aria-label="t('findings.export.menu')"
        :title="t('findings.export.menu')"
        data-testid="export-menu"
      />
    </UDropdownMenu>
  </UFieldGroup>
</template>
