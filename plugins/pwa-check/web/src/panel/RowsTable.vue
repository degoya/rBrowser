<script setup lang="ts">
// A table of label and value rows (RB-080-85): the manifest's head and members, the service
// worker. With `anchors`, each label is the place a finding jumps to (`field-<label>`).
import { computed } from 'vue';

const props = defineProps<{ rows: [string, string][]; head: [string, string]; anchors?: boolean }>();

const data = computed(() => props.rows.map(([label, value]) => ({ label, value })));
const columns = computed(() => [
  { accessorKey: 'label', header: props.head[0], meta: { class: { td: 'w-1/3 align-top font-medium text-highlighted' } } },
  { accessorKey: 'value', header: props.head[1], meta: { class: { td: 'break-all whitespace-normal' } } },
]);
</script>

<template>
  <UTable :data="data" :columns="columns" class="text-sm">
    <template #label-cell="{ row }">
      <span
        :id="anchors ? `field-${row.original.label}` : undefined"
        :tabindex="anchors ? -1 : undefined"
        class="rounded-sm outline-none focus-visible:ring-2 focus-visible:ring-primary"
        >{{ row.original.label }}</span
      >
    </template>
  </UTable>
</template>
