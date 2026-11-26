<script setup lang="ts">
// What the project turned off over the person's settings (RB-060-01): the rules it switched off
// and its ignore entries, folded under one note of the findings panel. Out of
// `FindingsPanel.vue` (RB-099-10, its length).
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UCollapsible from '@nuxt/ui/components/Collapsible.vue';
import { FINDINGS_ICONS } from './icons';
import type { IgnoreEntry, ProjectOff } from '../ipc/protocol';

const props = defineProps<{ projectOff: ProjectOff | null }>();
const { t } = useI18n();
const open = ref(false);
const count = computed(() => (props.projectOff === null ? 0 : props.projectOff.rules.length + props.projectOff.ignore.length));

/** An ignore entry as one line: its globs, `*` for one left out. */
function ignoreText(entry: IgnoreEntry): string {
  return [entry.rule, entry.url, entry.selector].map((glob) => glob ?? '*').join(' \u00b7 ');
}
</script>

<template>
  <UCollapsible v-if="count > 0 && projectOff" v-model:open="open" class="text-sm text-muted" data-testid="project-off">
    <UButton
      size="sm"
      variant="link"
      color="neutral"
      class="px-0"
      data-testid="project-off-toggle"
      :trailing-icon="FINDINGS_ICONS.checkDetails"
      :ui="{ trailingIcon: open ? 'rotate-180 transition-transform' : 'transition-transform' }"
    >
      {{ t('findings.project_off.note', { count: String(count) }) }}
    </UButton>
    <template #content>
      <ul class="mt-1 list-disc pl-5">
        <li v-for="rule in projectOff.rules" :key="`rule-${rule}`">{{ t('findings.project_off.rule', { rule }) }}</li>
        <li v-for="(entry, index) in projectOff.ignore" :key="`ignore-${index}`">
          {{ t('findings.project_off.ignore', { entry: ignoreText(entry) }) }}
        </li>
      </ul>
    </template>
  </UCollapsible>
</template>
