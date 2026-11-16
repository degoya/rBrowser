<script setup lang="ts">
// The desync badge of one view (RB-020-02), in its cell's label strip - the grid gap above the
// view, never over it. The tooltip says why the view could not follow; a click resyncs it (it
// loads the active view's page). An excluded view shows "not synced" instead.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { ViewDesync } from '../ipc/protocol';

const props = defineProps<{ desync: ViewDesync | null; excluded: boolean }>();
const emit = defineEmits<{ resync: [view: number] }>();
const { t } = useI18n();

const reason = computed(() =>
  props.desync === null
    ? ''
    : t(`sync.badge.reason.${props.desync.reason}`, {
        event: props.desync.event,
        selector: props.desync.selector ?? '',
      }),
);
</script>

<template>
  <button
    v-if="desync !== null"
    type="button"
    class="shrink-0 rounded-sm bg-warning/15 px-1 font-medium text-warning focus-visible:outline-2 focus-visible:outline-primary"
    :title="reason"
    data-testid="desync-badge"
    @click="emit('resync', desync.view)"
  >
    {{ t('sync.badge.desynced') }}
  </button>
  <span
    v-else-if="excluded"
    class="shrink-0 text-dimmed"
    :title="t('sync.badge.excludedHint')"
    data-testid="sync-excluded"
  >
    {{ t('sync.badge.excluded') }}
  </span>
</template>
