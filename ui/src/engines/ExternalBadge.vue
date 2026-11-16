<script setup lang="ts">
// The external view's badge (RB-040-01), in its cell's label strip - the grid gap above the view,
// never over it: the engine and the status ("Firefox: engine stopped", "Firefox: in its cell"
// while its window sits in the cell, RB-080-39), the reason or what the engine does not emulate
// as its tooltip, and the buttons "Show window" (running as a window of its own), "Restart"
// (stopped, failed) and "Open again" (a window the person closed stays closed, RB-080-38).
// Hidden with `engines.external.showTiles` off (the core sends no externals).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { ExternalView } from '../ipc/protocol';

const props = defineProps<{ external: ExternalView | null }>();
const emit = defineEmits<{ restart: [view: number]; show: [view: number] }>();
const { t } = useI18n();

const engine = computed(() => (props.external === null ? '' : t(`engines.name.${props.external.engine}`)));
/** The window sits in its grid cell (RB-080-39). */
const inCell = computed(() => props.external?.status === 'running' && props.external.embedded === true);
const chip = computed(() =>
  props.external === null
    ? ''
    : t('engines.external.chip', {
        engine: engine.value,
        status: inCell.value
          ? t('engines.external.status.embedded')
          : t(`engines.external.status.${props.external.status}`),
      }),
);
const hint = computed(() => {
  const external = props.external;
  if (external === null) return '';
  const unsupported = external.unsupported ?? [];
  if (external.status === 'running' && unsupported.length > 0) {
    return t('engines.external.unsupported', { engine: engine.value, fields: unsupported.join(', ') });
  }
  if (inCell.value) return t('engines.external.hint.embedded', { engine: engine.value });
  return t(`engines.external.hint.${external.status}`, {
    engine: engine.value,
    reason: external.reason ?? '',
  });
});
const canRestart = computed(
  () =>
    props.external?.status === 'exited' ||
    props.external?.status === 'failed' ||
    props.external?.status === 'closed',
);
const restartLabel = computed(() =>
  props.external?.status === 'closed' ? t('engines.external.reopen') : t('engines.external.restart'),
);
const canShow = computed(() => props.external?.status === 'running' && !inCell.value);
const tone = computed(() => {
  switch (props.external?.status) {
    case 'exited':
    case 'failed':
      return 'bg-error/15 text-error';
    case 'closed':
      return 'bg-elevated text-muted';
    case 'running':
      return 'bg-primary/15 text-primary';
    default:
      return 'bg-elevated text-muted';
  }
});
</script>

<template>
  <span v-if="external !== null" class="flex shrink-0 items-center gap-1" data-testid="external-badge">
    <span
      class="rounded-sm px-1 font-medium"
      :class="tone"
      :title="hint"
      :aria-label="`${chip}. ${hint}`"
      role="status"
      data-testid="external-status"
    >
      {{ chip }}
    </span>
    <button
      v-if="canShow"
      type="button"
      class="rounded-sm px-1 underline focus-visible:outline-2 focus-visible:outline-primary"
      data-testid="external-show"
      @click="emit('show', external.view)"
    >
      {{ t('engines.external.show') }}
    </button>
    <button
      v-if="canRestart"
      type="button"
      class="rounded-sm px-1 underline focus-visible:outline-2 focus-visible:outline-primary"
      data-testid="external-restart"
      @click="emit('restart', external.view)"
    >
      {{ restartLabel }}
    </button>
  </span>
</template>
