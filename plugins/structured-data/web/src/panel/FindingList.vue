<script setup lang="ts">
// The structured data panel's findings (RB-090-96): the summary, then a table of the findings,
// errors first - the severity as a badge with its word, the sentence as a button that shows the
// finding's node in the tree.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Answer, type Finding, findingText, SEVERITY_ICON, sortedFindings, summaryText, text } from './model';

const props = defineProps<{ answer: Answer }>();
const emit = defineEmits<{ jump: [finding: Finding] }>();
const { strings } = usePanel();

const list = computed(() => sortedFindings(props.answer));
const columns = computed(() => [
  { id: 'severity', header: text(strings.value, 'panel.severity.label'), meta: { class: { td: 'w-24 align-top' } } },
  { id: 'finding', header: text(strings.value, 'panel.finding') },
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
          :data-tone="(row.original as Finding).severity"
          :color="(row.original as Finding).severity"
          :icon="SEVERITY_ICON[(row.original as Finding).severity]"
          variant="subtle"
          :label="text(strings, `panel.severity.${(row.original as Finding).severity}`)"
        />
      </template>
      <template #finding-cell="{ row }">
        <UButton
          v-if="(row.original as Finding).node"
          data-finding
          color="neutral"
          variant="link"
          class="h-auto p-0 text-left whitespace-normal"
          :label="findingText(strings, row.original as Finding)"
          :title="text(strings, 'panel.jump')"
          @click="emit('jump', row.original as Finding)"
        />
        <span v-else data-finding class="whitespace-normal">{{ findingText(strings, row.original as Finding) }}</span>
      </template>
    </UTable>
  </div>
</template>
