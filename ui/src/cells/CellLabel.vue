<script setup lang="ts">
// The label strip of one grid cell, in the gap above its view: breakpoint label, size
// (`393 × 852 @3x`, the click target of the size quick pick of RB-010-16), the fit zoom, room for
// the address field of a view whose navigation is decoupled (RB-020-06), the badges - desync
// (RB-020), external engine (RB-040-01), Web Vitals and resources (RB-060-10) - and the session
// icon.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { ViewInfo } from '../ipc/protocol';
import { formatCellSize, formatScale } from './format';

// `sessionColor`: the badge colour of the view's session (RB-020-07), from the state's sessions.
const props = defineProps<{ view: ViewInfo; active: boolean; sessionColor?: string | null }>();
const emit = defineEmits<{ pickSize: [view: number]; activate: [view: number] }>();
const { t } = useI18n();

const size = computed(() => formatCellSize(props.view.width, props.view.height, props.view.dpr));
const scale = computed(() => formatScale(props.view.scale));
</script>

<template>
  <div
    class="flex h-full min-w-0 items-center gap-2 overflow-hidden px-1 text-xs text-muted"
    :class="active ? 'text-highlighted' : ''"
    :aria-current="active ? 'true' : undefined"
    :aria-label="t('shell.cell.label', { label: view.label, size })"
    role="group"
    data-testid="cell-label"
  >
    <button
      type="button"
      class="truncate rounded-sm font-medium focus-visible:outline-2 focus-visible:outline-primary"
      :title="active ? t('shell.cell.active') : view.title"
      data-testid="cell-name"
      @click="emit('activate', view.id)"
    >
      {{ view.label }}
    </button>
    <button
      type="button"
      class="shrink-0 rounded-sm tabular-nums focus-visible:outline-2 focus-visible:outline-primary"
      data-testid="cell-size"
      @click="emit('pickSize', view.id)"
    >
      {{ size }}
    </button>
    <span v-if="scale !== null" class="shrink-0 tabular-nums" data-testid="cell-scale">
      {{ t('shell.cell.scale', { percent: scale }) }}
    </span>
    <span v-if="view.loading" class="sr-only">{{ t('shell.cell.loading') }}</span>
    <!-- The view's own address field when its navigation is decoupled (RB-020-06). -->
    <slot name="address" />
    <!-- The badges from CellLabelApp (desync RB-020-02, engine RB-040-01, Web Vitals RB-060-10)
         and the session badge (RB-020-07). -->
    <span class="ml-auto flex shrink-0 items-center gap-1" data-testid="cell-badges">
      <slot name="badges" />
      <span
        v-if="view.session !== 'default'"
        class="flex items-center gap-1 truncate"
        :style="sessionColor ? { color: sessionColor } : undefined"
        data-testid="cell-session"
      >
        <span aria-hidden="true">&#9679;</span>
        {{ t('shell.cell.session', { session: view.session }) }}
      </span>
    </span>
  </div>
</template>
