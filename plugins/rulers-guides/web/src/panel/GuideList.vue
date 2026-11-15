<script setup lang="ts">
// The guides (RB-060-05, on Nuxt UI since RB-080-86): a row per guide - its axis, what it is
// measured from (a horizontal guide only from the top), its position typed exactly and a remove
// button - then Add vertical and Add horizontal (a new guide at 100 px). Every change sends all
// guides as one `change`; a cleared or unreadable position sends nothing.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import { guidesArg, unitKey, unitsOf, type Axis, type Guide, type Unit } from './model';

const props = defineProps<{ guides: Guide[] }>();
const emit = defineEmits<{ change: [args: Record<string, unknown>] }>();
const { t } = usePanel();

const axisText = (axis: Axis): string => t(axis === 'x' ? 'panel.axis_x' : 'panel.axis_y');
const rowName = (guide: Guide): string => t('panel.guide_row', { id: guide.id, axis: axisText(guide.axis) });
const units = (axis: Axis) => unitsOf(axis).map((value) => ({ value, label: t(unitKey(axis, value)) }));

function send(guides: Guide[]): void {
  emit('change', guidesArg(guides));
}

function setUnit(index: number, unit: unknown): void {
  send(props.guides.map((guide, at) => (at === index ? { ...guide, unit: unit as Unit } : guide)));
}

function setValue(index: number, event: Event): void {
  const text = (event.target as HTMLInputElement).value;
  const value = Number(text);
  if (text === '' || !Number.isFinite(value)) return;
  send(props.guides.map((guide, at) => (at === index ? { ...guide, value } : guide)));
}

function add(axis: Axis): void {
  send([...props.guides, { id: 0, axis, unit: 'px', value: 100 }]);
}
</script>

<template>
  <section aria-labelledby="guides-title" class="flex flex-col gap-2">
    <h2 id="guides-title" class="text-sm font-semibold text-highlighted">{{ t('panel.guides') }}</h2>
    <p v-if="guides.length === 0" id="guides-empty" class="text-sm text-muted">{{ t('panel.guides_empty') }}</p>
    <ul v-else id="guides" class="flex flex-col divide-y divide-default rounded-md border border-default">
      <li v-for="(guide, index) in guides" :key="index" class="flex items-center gap-2 px-2 py-1.5" :data-guide="guide.id">
        <UBadge
          color="neutral"
          variant="outline"
          size="sm"
          :icon="guide.axis === 'x' ? 'i-lucide-separator-vertical' : 'i-lucide-separator-horizontal'"
          :label="axisText(guide.axis)"
          class="w-24 shrink-0"
          :ui="{ label: 'truncate' }"
          :title="rowName(guide)"
          data-axis
        />
        <USelect
          :model-value="guide.unit"
          :items="units(guide.axis)"
          :disabled="guide.axis === 'y'"
          :data-key="`unit-${index}`"
          :aria-label="`${rowName(guide)}: ${t('panel.unit')}`"
          size="sm"
          class="min-w-0 flex-1"
          @update:model-value="(unit: unknown) => setUnit(index, unit)"
        />
        <UInput
          :model-value="String(guide.value)"
          type="number"
          min="0"
          :max="guide.unit === 'percent' ? '100' : '100000'"
          step="any"
          :data-key="`value-${index}`"
          :aria-label="`${rowName(guide)}: ${t('panel.value')}`"
          size="sm"
          class="w-20 shrink-0"
          @change="(event: Event) => setValue(index, event)"
        />
        <IconAction :data-key="`remove-${index}`" icon="i-lucide-trash-2" :label="t('panel.remove_guide', { id: guide.id })" @click="send(guides.filter((_, at) => at !== index))" />
      </li>
    </ul>
    <div class="flex flex-wrap gap-2">
      <UButton id="add-x" icon="i-lucide-separator-vertical" color="neutral" variant="outline" size="sm" :label="t('panel.add_vertical')" @click="add('x')" />
      <UButton id="add-y" icon="i-lucide-separator-horizontal" color="neutral" variant="outline" size="sm" :label="t('panel.add_horizontal')" @click="add('y')" />
    </div>
  </section>
</template>
