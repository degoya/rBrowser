<script setup lang="ts">
// A list of names and values (RB-096-01): the query's parameters, the headers, a form's fields.
// Each row edits in place; with `toggles`, a row can be switched off and is kept but not sent.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import type { Entry } from './model';

defineProps<{ id: string; toggles?: boolean }>();
const rows = defineModel<Entry[]>({ required: true });
const t = usePanel().t;

function update(index: number, patch: Partial<Entry>): void {
  rows.value = rows.value.map((row, at) => (at === index ? { ...row, ...patch } : row));
}

function add(): void {
  rows.value = [...rows.value, { name: '', value: '', enabled: true }];
}

function remove(index: number): void {
  rows.value = rows.value.filter((_, at) => at !== index);
}
</script>

<template>
  <div :id="id" class="flex flex-col gap-1.5">
    <div v-for="(row, index) in rows" :key="index" data-row class="flex items-center gap-1.5">
      <UCheckbox
        v-if="toggles"
        :model-value="row.enabled"
        :aria-label="t('panel.table.enabled')"
        @update:model-value="(on) => update(index, { enabled: on === true })"
      />
      <UInput
        :model-value="row.name"
        size="sm"
        class="min-w-0 flex-1 font-mono"
        spellcheck="false"
        autocomplete="off"
        :placeholder="t('panel.table.name')"
        :aria-label="t('panel.table.name')"
        @update:model-value="(text) => update(index, { name: String(text) })"
      />
      <UInput
        :model-value="row.value"
        size="sm"
        class="min-w-0 flex-[2] font-mono"
        spellcheck="false"
        autocomplete="off"
        :placeholder="t('panel.table.value')"
        :aria-label="t('panel.table.value')"
        @update:model-value="(text) => update(index, { value: String(text) })"
      />
      <IconAction icon="i-lucide-x" :label="t('panel.table.remove')" @click="remove(index)" />
    </div>
    <div>
      <UButton data-add size="xs" color="neutral" variant="ghost" icon="i-lucide-plus" :label="t('panel.table.add')" @click="add" />
    </div>
  </div>
</template>
