<script setup lang="ts">
// The fields of one form (RB-080-86): a row per field - its name, a star when required, its kind,
// the value filled once, each on one line and cut with the whole text as its title. A field that
// differs between views says in how many and opens to the value per view; the fields skipped in
// every view are folded together below, each with its reasons.
import { computed, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { fieldName, type FieldRow, marks, reasonText, type Section, viewText } from './model';

const props = defineProps<{ section: Section }>();
const { t } = usePanel();

const expanded = ref<Record<string, boolean>>({});
// Another set of fields closes the rows; the same fields again (new texts, a theme) keep them open.
watch(
  () => props.section.rows.map((row) => row.key).join('\n'),
  () => (expanded.value = {}),
);

const columns = computed(() => [
  { id: 'field', header: t('panel.field'), meta: { class: { th: 'w-2/5', td: 'w-2/5 align-top' } } },
  { id: 'value', header: t('panel.value'), meta: { class: { td: 'align-top' } } },
]);
const skippedItems = computed(() => [
  { label: t('panel.skippedFields', { count: props.section.skipped.length }), icon: 'i-lucide-eye-off', value: 'skipped' },
]);
const kindTitle = (kind: string): string => `${t('panel.kind')}: ${kind}`;
const rowOf = (original: unknown): FieldRow => original as FieldRow;
</script>

<template>
  <div class="flex flex-col gap-2">
    <UTable
      v-if="section.rows.length > 0"
      v-model:expanded="expanded"
      data-fields
      :data="section.rows"
      :columns="columns"
      :ui="{ base: 'table-fixed', th: 'px-2 py-1.5', td: 'px-2 py-1.5 whitespace-normal' }"
      class="text-sm"
    >
      <template #field-cell="{ row }">
        <div data-name class="flex min-w-0 items-center gap-1">
          <span class="truncate font-medium text-highlighted" :title="rowOf(row.original).field.label">{{ fieldName(rowOf(row.original).field).text }}</span>
          <template v-if="fieldName(rowOf(row.original).field).required">
            <span class="shrink-0 text-error" aria-hidden="true">*</span>
            <span class="sr-only">{{ t('panel.required') }}</span>
          </template>
        </div>
        <UBadge data-kind class="mt-0.5 max-w-full" color="neutral" variant="subtle" size="sm" :title="kindTitle(rowOf(row.original).field.kind)">
          <span class="truncate">{{ rowOf(row.original).field.kind }}</span>
        </UBadge>
      </template>
      <template #value-cell="{ row }">
        <div class="flex min-w-0 flex-col gap-1">
          <span v-if="rowOf(row.original).value" data-value class="truncate text-default" :title="rowOf(row.original).value">{{ rowOf(row.original).value }}</span>
          <span v-else data-value class="truncate italic text-muted">{{ t('panel.emptyValue') }}</span>
          <div class="flex flex-wrap items-center gap-1">
            <UBadge v-for="mark in marks(t, rowOf(row.original).field)" :key="mark.text" data-mark :color="mark.color" variant="subtle" size="sm" :label="mark.text" />
            <UButton
              v-if="rowOf(row.original).differs > 0"
              data-differs
              size="xs"
              color="warning"
              variant="soft"
              :trailing-icon="row.getIsExpanded() ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
              :aria-expanded="row.getIsExpanded() ? 'true' : 'false'"
              :label="t('panel.differs', { count: rowOf(row.original).differs })"
              @click="row.toggleExpanded()"
            />
          </div>
        </div>
      </template>
      <template #expanded="{ row }">
        <ul data-per-view class="flex flex-col gap-0.5 ps-2 text-xs" :aria-label="t('panel.perView')">
          <li v-for="cell in rowOf(row.original).cells" :key="cell.view.view" class="flex min-w-0 gap-2" :data-same="cell.same ? '' : undefined">
            <span class="w-24 shrink-0 truncate font-medium" :title="cell.view.label">{{ cell.view.label }}</span>
            <span class="truncate" :class="cell.same ? 'text-muted' : 'text-highlighted'" :title="viewText(t, cell)">{{ viewText(t, cell) }}</span>
          </li>
        </ul>
      </template>
    </UTable>

    <UAccordion
      v-if="section.skipped.length > 0"
      data-skipped
      class="px-2"
      :items="skippedItems"
      :unmount-on-hide="false"
      :ui="{ trigger: 'py-2 text-sm', body: 'pb-2' }"
    >
      <template #body>
        <ul class="flex flex-col divide-y divide-default text-sm">
          <li v-for="skip in section.skipped" :key="skip.key" data-skipped-field class="flex min-w-0 items-center gap-1.5 py-1">
            <span class="truncate" :title="skip.field.label">{{ fieldName(skip.field).text }}</span>
            <template v-if="fieldName(skip.field).required">
              <span class="shrink-0 text-error" aria-hidden="true">*</span>
              <span class="sr-only">{{ t('panel.required') }}</span>
            </template>
            <UBadge class="shrink-0" color="neutral" variant="subtle" size="sm" :title="kindTitle(skip.field.kind)" :label="skip.field.kind" />
            <span class="ms-auto flex shrink-0 gap-1">
              <UBadge v-for="reason in skip.reasons" :key="reason" data-reason color="neutral" variant="outline" size="sm" :label="reasonText(t, reason)" />
            </span>
          </li>
        </ul>
      </template>
    </UAccordion>
  </div>
</template>
