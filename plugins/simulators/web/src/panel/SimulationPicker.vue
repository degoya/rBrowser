<script setup lang="ts">
// The simulations to choose from (RB-080-86): Off, then the catalogue's groups, each a radio
// group with its name as the legend and every simulation with its description. Shift held while a
// simulation is chosen - by a click or by the keyboard - asks for every view; Enter on a
// simulation applies it again (Shift+Enter in every view).
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Entry, groups, typeName } from './model';

const props = defineProps<{ catalog: readonly Entry[] }>();
const selection = defineModel<string>({ required: true });
const emit = defineEmits<{ pick: [type: string, shift: boolean]; enter: [shift: boolean] }>();
const t = usePanel().t;

const sections = computed(() =>
  groups(props.catalog).map((group) => ({
    id: group.id,
    legend: group.id === 'none' ? undefined : t(`groups.${group.id}`),
    items: group.entries.map((entry) => ({ value: entry.id, label: typeName(t, entry.id), description: t(`types.${entry.id}.description`) })),
  })),
);

let shift = false;
// A pointer's click says whether Shift was held; the click a radio gives itself when the arrow
// keys move to it (detail 0) does not, the key did.
function onClick(event: MouseEvent): void {
  if (event.detail > 0) shift = event.shiftKey;
}
function onKey(event: KeyboardEvent): void {
  shift = event.shiftKey;
  if (event.key === 'Enter' && (event.target as HTMLElement).matches('[role="radio"]')) {
    event.preventDefault();
    emit('enter', event.shiftKey);
  }
}
function changed(value: unknown): void {
  if (typeof value !== 'string') return;
  selection.value = value;
  emit('pick', value, shift);
  shift = false;
}
</script>

<template>
  <div id="groups" class="flex flex-col gap-4" @click.capture="onClick" @keydown.capture="onKey">
    <URadioGroup
      v-for="section in sections"
      :key="section.id"
      :data-group="section.id"
      :legend="section.legend"
      :items="section.items"
      :model-value="selection"
      size="sm"
      :ui="{ legend: 'mb-1.5 text-xs font-semibold uppercase text-toned', fieldset: 'gap-y-2' }"
      @update:model-value="changed"
    />
  </div>
</template>
