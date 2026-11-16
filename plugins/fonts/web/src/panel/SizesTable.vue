<script setup lang="ts">
// The font sizes per breakpoint (RB-080-83): a row per text role, the largest size in each view
// that has one (a Firefox view has none and was a column of dashes), and "not scaled" and
// "jumps" as badges in a column of their own, not beside the role's name.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Answer, roleCells, roleViews } from './model';

const props = defineProps<{ answer: Answer }>();
const { t } = usePanel();

const views = computed(() => roleViews(props.answer));
const data = computed(() =>
  props.answer.roles.map((series) => ({
    role: series.role,
    notes: [...(series.scaled === false ? [t('panel.notScaled')] : []), ...(series.jump ? [t('panel.jump')] : [])],
    ...Object.fromEntries(roleCells(props.answer, series, views.value).map((cell, index) => [`v${index}`, cell])),
  })),
);
const columns = computed(() => [
  { accessorKey: 'role', header: t('panel.role') },
  ...views.value.map((view, index) => ({
    accessorKey: `v${index}`,
    header: view.label,
    meta: { class: { th: 'text-right', td: 'text-right tabular-nums' } },
  })),
  { id: 'notes', header: t('panel.flags') },
]);
</script>

<template>
  <section id="roles" :aria-label="t('panel.breakpoints')">
    <h2 class="mb-1 flex items-center gap-1.5 text-xs font-semibold uppercase text-toned">
      <UIcon name="i-lucide-type" class="size-4" />
      <span>{{ t('panel.breakpoints') }}</span>
    </h2>
    <UTable :data="data" :columns="columns" class="text-sm">
      <template #role-cell="{ row }">
        <span class="font-medium">{{ row.original.role }}</span>
      </template>
      <template #notes-cell="{ row }">
        <div class="flex flex-wrap gap-1">
          <UBadge v-for="note in row.original.notes" :key="note" color="warning" variant="subtle" size="sm" data-tone="warn" :label="note" />
        </div>
      </template>
    </UTable>
  </section>
</template>
