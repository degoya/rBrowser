<script setup lang="ts">
// The views a test run takes (RB-099-10), one pick list for every start point of the shell - the
// findings panel's Run checks, its visual regression and the Flows panel: every view, the views
// the grid shows, or chosen breakpoints (a multiple choice of the project's breakpoints). The model
// is panel-ui's `scope.ts`, which the Lighthouse panel's pick list shares; `useScope.ts` keeps the
// last choice per function. A start point that runs in one view too (Flows) hands in its `views`:
// each is a choice of the same list, `view` its id (a view of a free size included). Nuxt UI only
// (docs/design.md), a pick list, no free text.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UFormField from '@nuxt/ui/components/FormField.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import USelectMenu from '@nuxt/ui/components/SelectMenu.vue';
import { SCOPE_MODES, type ScopeChoice, type ScopeMode } from '../../../plugins/web-common/panel-ui/src/scope';

const props = withDefaults(
  defineProps<{
    modelValue: ScopeChoice;
    /** The project's breakpoints, in config order. */
    breakpoints: { id: string; label?: string }[];
    disabled?: boolean;
    /** The field's name where the start point has its own ("Run in"). */
    label?: string;
    /** The single views offered beside the scopes; none by default. */
    views?: { id: number; label: string }[];
    /** The single view chosen instead of a scope (`v-model:view`). */
    view?: number | null;
  }>(),
  { disabled: false, label: undefined, views: () => [], view: null },
);
const emit = defineEmits<{ 'update:modelValue': [choice: ScopeChoice]; 'update:view': [view: number | null] }>();

/** A single view's entry in the list. */
const VIEW_PREFIX = 'view:';
const { t } = useI18n();

const modes = computed(() => [
  ...SCOPE_MODES.map((mode) => ({ value: mode as string, label: t(`scope.picker.${mode}`) })),
  ...props.views.map((view) => ({ value: `${VIEW_PREFIX}${view.id}`, label: view.label })),
]);
const shown = computed(() => (props.view === null ? props.modelValue.mode : `${VIEW_PREFIX}${props.view}`));
const choices = computed(() => props.breakpoints.map((breakpoint) => ({ value: breakpoint.id, label: breakpoint.label ?? breakpoint.id })));
const empty = computed(() => props.view === null && props.modelValue.mode === 'chosen' && props.modelValue.breakpoints.length === 0);

function chooseMode(value: string): void {
  if (value.startsWith(VIEW_PREFIX)) {
    emit('update:view', Number(value.slice(VIEW_PREFIX.length)));
    return;
  }
  const mode = value as ScopeMode;
  if (props.view !== null) {
    emit('update:view', null);
  }
  emit('update:modelValue', { mode, breakpoints: mode === 'chosen' ? props.modelValue.breakpoints : [] });
}

function chooseBreakpoints(ids: string[]): void {
  emit('update:modelValue', { mode: 'chosen', breakpoints: ids });
}
</script>

<template>
  <div class="flex flex-wrap items-end gap-2" data-testid="scope-picker">
    <UFormField :label="label ?? t('scope.picker.label')" size="sm">
      <USelect
        :model-value="shown"
        :items="modes"
        :disabled="disabled"
        size="sm"
        class="min-w-36"
        data-testid="scope-mode"
        @update:model-value="(value: string) => chooseMode(value)"
      />
    </UFormField>
    <UFormField v-if="view === null && modelValue.mode === 'chosen'" :label="t('scope.picker.breakpoints')" size="sm">
      <USelectMenu
        :model-value="modelValue.breakpoints"
        :items="choices"
        value-key="value"
        multiple
        :disabled="disabled"
        size="sm"
        class="min-w-44"
        :placeholder="t('scope.picker.pick')"
        data-testid="scope-breakpoints"
        @update:model-value="(ids: string[]) => chooseBreakpoints(ids)"
      />
    </UFormField>
    <p v-if="empty" class="text-xs text-warning" role="status" data-testid="scope-empty">{{ t('scope.picker.none') }}</p>
  </div>
</template>
