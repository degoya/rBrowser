<script setup lang="ts">
// The PWA panel's findings (RB-080-08, on Nuxt UI since RB-080-85): the summary, then a table of
// the page's findings, errors first - the severity as a badge with its word, the message as a
// button that jumps to the finding's place in the other tabs.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Answer, type Finding, findingsOf, findingText, type PageReport, summaryText, text } from './model';

const props = defineProps<{ answer: Answer; page: PageReport }>();
const emit = defineEmits<{ jump: [finding: Finding] }>();
const { strings } = usePanel();

const COLOR = { error: 'error', warning: 'warning', info: 'info' } as const;
const TONE = { error: 'error', warning: 'warn', info: 'info' } as const;

const list = computed(() => findingsOf(props.answer, props.page));
const columns = computed(() => [
  { id: 'severity', header: text(strings.value, 'panel.table.severity'), meta: { class: { td: 'w-24 align-top' } } },
  { id: 'finding', header: text(strings.value, 'panel.table.finding') },
]);
</script>

<template>
  <div class="flex flex-col gap-2">
    <p id="summary" class="text-sm text-muted">{{ summaryText(strings, answer) }}</p>
    <p v-if="list.length === 0" id="no-findings" class="flex items-center gap-1.5 text-sm text-muted">
      <UIcon name="i-lucide-circle-check" class="size-4 shrink-0 text-success" />
      <span>{{ text(strings, 'panel.noFindings') }}</span>
    </p>
    <UTable v-else id="findings-table" :data="list" :columns="columns" class="text-sm">
      <template #severity-cell="{ row }">
        <UBadge
          data-severity
          :data-tone="TONE[(row.original as Finding).severity]"
          :color="COLOR[(row.original as Finding).severity]"
          variant="subtle"
          :label="text(strings, `panel.severity.${(row.original as Finding).severity}`)"
        />
      </template>
      <template #finding-cell="{ row }">
        <UButton
          data-finding
          color="neutral"
          variant="link"
          class="h-auto p-0 text-left whitespace-normal"
          :label="findingText(strings, row.original as Finding)"
          @click="emit('jump', row.original as Finding)"
        />
      </template>
    </UTable>
  </div>
</template>
