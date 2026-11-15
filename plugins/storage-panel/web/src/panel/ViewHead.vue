<script setup lang="ts">
// A view's column head (RB-070-01, on Nuxt UI since RB-080-84): its label, breakpoint and engine;
// greyed with a note for a view the panel cannot read (another engine, or a failed read), the
// page's address in the tooltip otherwise.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { StorageView } from './model';

const props = defineProps<{ view: StorageView }>();
const panel = usePanel();
/** A failure code as its sentence; free text as it is. */
const said = (error: string): string => (/^[a-z][\w-]*(\.[\w-]+)+( |$)/.test(error) ? panel.error(error) : error);
const hint = computed(() => {
  if (props.view.status === 'unsupported') return panel.t('panel.engineHint', { engine: props.view.engine });
  if (props.view.status === 'error') return panel.t('panel.viewError', { text: said(props.view.error ?? '') });
  return props.view.url;
});
</script>

<template>
  <UTooltip :text="hint || undefined" :disabled="!hint">
    <span data-view-head :data-off="view.status === 'ok' ? undefined : ''" :data-hint="hint" class="flex flex-col" :class="view.status === 'ok' ? '' : 'text-muted'">
      <span data-label class="flex items-center gap-1 font-semibold">
        <UIcon v-if="view.status !== 'ok'" name="i-lucide-eye-off" class="size-3.5" />{{ view.label }}
      </span>
      <span class="text-xs font-normal text-muted">{{ view.breakpoint + ' · ' + view.engine }}</span>
    </span>
  </UTooltip>
</template>
