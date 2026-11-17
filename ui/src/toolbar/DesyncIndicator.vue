<script setup lang="ts">
// The toolbar's desync indicator (RB-090-23): shown while views could not follow the active view
// (RB-020-02). The label strips are native and carry no desync badge (`cell-label.html` was
// removed, RB-090-27), so this is where the state is visible. A click runs `sync.resync` for
// every desynced view. Its name is its visible label (WCAG 2.5.3); the views it names are its
// description (`title`).
// Gone because the views follow again, it hands the focus to its group's first control, the sync
// switch.
import { computed, nextTick, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { useActivities } from '../activity/store';
import { useShell } from '../state/shell';

const shell = useShell();
const activities = useActivities();
const { t } = useI18n();

// The compact form of the activity bar's desync entry: gone while the entry is in the bar
// (RB-090-67).
const desynced = computed(() => (activities.inBar('desync') ? [] : (shell.state.value?.sync.desynced ?? [])));
const views = computed(() =>
  desynced.value
    .map((entry) => shell.state.value?.views.find((view) => view.id === entry.view)?.label ?? String(entry.view))
    .join(', '),
);
const button = ref<{ $el: HTMLElement } | null>(null);

watch(
  () => desynced.value.length > 0,
  async (shown) => {
    const element = button.value?.$el;
    if (shown || element === undefined || element !== document.activeElement) {
      return;
    }
    const group = element.closest('[data-toolbar-group]');
    await nextTick();
    group?.querySelector<HTMLElement>('button:not(:disabled)')?.focus();
  },
  { flush: 'pre' },
);

function resync(): void {
  void shell.send({ type: 'runCommand', id: 'sync.resync' });
}
</script>

<template>
  <UButton
    v-if="desynced.length > 0"
    ref="button"
    color="warning"
    variant="soft"
    size="sm"
    class="max-w-40 shrink-0 truncate"
    :label="t('sync.badge.indicator', { count: desynced.length })"
    :title="t('sync.badge.indicatorTitle', { views })"
    data-testid="desync-indicator"
    @click="resync"
  />
</template>
