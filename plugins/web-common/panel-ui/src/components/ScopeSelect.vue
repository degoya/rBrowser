<script setup lang="ts">
// The views a plugin panel's run takes (RB-099-10): every view, the views the grid shows, or
// chosen breakpoints - the pick list of the shell's `ScopePicker.vue` for a panel, on the same
// model (`../scope.ts`). The panel hands in its texts (its own locale keys) and the breakpoints
// it knows; it sends the scope as `$scope`, which the host resolves into `breakpoints`.
import { computed } from 'vue';
import { SCOPE_MODES, type ScopeChoice, type ScopeMode, type ScopeTexts } from '../scope';

const props = withDefaults(
  defineProps<{
    modelValue: ScopeChoice;
    breakpoints: { id: string; label?: string }[];
    texts: ScopeTexts;
    disabled?: boolean;
  }>(),
  { disabled: false },
);
const emit = defineEmits<{ 'update:modelValue': [choice: ScopeChoice] }>();

const modes = computed(() => SCOPE_MODES.map((mode) => ({ value: mode, label: props.texts[mode] })));
const choices = computed(() => props.breakpoints.map((breakpoint) => ({ value: breakpoint.id, label: breakpoint.label ?? breakpoint.id })));
const empty = computed(() => props.modelValue.mode === 'chosen' && props.modelValue.breakpoints.length === 0);

function chooseMode(mode: ScopeMode): void {
  emit('update:modelValue', { mode, breakpoints: mode === 'chosen' ? props.modelValue.breakpoints : [] });
}
</script>

<template>
  <div class="flex flex-wrap items-end gap-2" data-testid="scope-select">
    <UFormField :label="texts.label" size="sm">
      <USelect
        :model-value="modelValue.mode"
        :items="modes"
        :disabled="disabled"
        size="sm"
        class="min-w-36"
        data-testid="scope-mode"
        @update:model-value="(mode: ScopeMode) => chooseMode(mode)"
      />
    </UFormField>
    <UFormField v-if="modelValue.mode === 'chosen'" :label="texts.breakpoints" size="sm">
      <USelectMenu
        :model-value="modelValue.breakpoints"
        :items="choices"
        value-key="value"
        multiple
        :disabled="disabled"
        size="sm"
        class="min-w-44"
        :placeholder="texts.pick"
        data-testid="scope-breakpoints"
        @update:model-value="(ids: string[]) => emit('update:modelValue', { mode: 'chosen', breakpoints: ids })"
      />
    </UFormField>
    <p v-if="empty" class="text-xs text-warning" role="status" data-testid="scope-empty">{{ texts.none }}</p>
  </div>
</template>
