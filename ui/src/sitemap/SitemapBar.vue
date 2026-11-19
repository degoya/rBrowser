<script setup lang="ts">
// The sitemap bar (RB-050-05): `panels.sitemap` (`Ctrl+Shift+Space P G`, or the toolbar's
// indicator) reaches the shell as `uiCommand`, and this bar covers the toolbar strip (the shell
// view, never a page) with the last capture: its progress and time left while it runs, its
// outcome afterwards. It also opens by itself when a capture starts or resumes, from wherever
// (RB-090-83: a start shows that something happens) - then without taking the focus - and stays
// until it is closed. Start, cancel, resume and "open the gallery" (`sitemap.gallery`: a window
// of its own, never a view) are `UButton`s (RB-080-88), the first one focused when the person
// opened the bar; Escape closes. The coordinator mounts it in `App.vue`. Closed by Escape,
// Cancel or done, the focus goes back to its opener (`returnFocus.ts`, RB-090-23).
import { computed, nextTick, onScopeDispose, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { returnFocus } from '../focus/returnFocus';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import {
  PANEL_COMMAND,
  cancelMessage,
  captureMessage,
  galleryMessage,
  isRunning,
  resumeMessage,
  sitemapOf,
  startsShowing,
  stateKey,
  timeLeft,
} from './sitemap';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();

const open = ref(false);
const bar = ref<HTMLElement | null>(null);
const focus = returnFocus();
const status = computed(() => sitemapOf(shell.state.value));
const running = computed(() => isRunning(status.value));
const line = computed(() => {
  const args = {
    done: status.value?.done ?? 0,
    total: status.value?.total ?? 0,
    failed: status.value?.failed ?? 0,
  };
  const text = t(stateKey(status.value), args);
  const left = running.value ? timeLeft(status.value?.etaSeconds) : null;
  return left === null ? text : `${text}, ${t(left.key, left.args)}`;
});

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'uiCommand' && message.id === PANEL_COMMAND) {
    focus.remember();
    open.value = true;
    void nextTick(() => bar.value?.querySelector<HTMLElement>('button')?.focus());
  }
});
onScopeDispose(stop);

// A capture that starts or resumes opens the bar, wherever it was started; the focus stays.
watch(status, (now, before) => {
  if (startsShowing(now, before)) {
    open.value = true;
  }
});

function close(): void {
  open.value = false;
  focus.restore();
}

function start(): void {
  void shell.send(captureMessage());
}

function cancel(): void {
  if (status.value !== null) {
    void shell.send(cancelMessage(status.value.runId));
  }
}

function resume(): void {
  if (status.value !== null) {
    void shell.send(resumeMessage(status.value.runId));
  }
}

function gallery(): void {
  if (status.value?.gallery !== undefined && status.value?.gallery !== null) {
    void shell.send(galleryMessage(status.value.runId));
    close();
  }
}
</script>

<template>
  <div
    v-if="open"
    ref="bar"
    role="dialog"
    :aria-label="t('shell.sitemap.label')"
    class="fixed inset-x-0 top-0 z-50 flex h-11 items-center gap-2 overflow-x-auto border-b border-default bg-elevated px-3 text-sm text-default"
    data-testid="sitemap-bar"
    @keydown.escape.prevent="close"
  >
    <span class="shrink-0 font-medium">{{ t('shell.sitemap.label') }}</span>
    <span class="min-w-0 truncate text-muted" role="status" data-testid="sitemap-line">{{ line }}</span>
    <UButton v-if="running" size="sm" color="error" class="shrink-0" data-testid="sitemap-cancel" @click="cancel">
      {{ t('shell.sitemap.cancel') }}
    </UButton>
    <template v-else>
      <UButton
        v-if="status?.state === 'cancelled'"
        size="sm"
        color="neutral"
        variant="outline"
        class="shrink-0"
        data-testid="sitemap-resume"
        @click="resume"
      >
        {{ t('shell.sitemap.resume') }}
      </UButton>
      <UButton size="sm" color="neutral" variant="outline" class="shrink-0" data-testid="sitemap-start" @click="start">
        {{ t('shell.sitemap.start') }}
      </UButton>
    </template>
    <UButton
      v-if="status?.gallery !== undefined && status?.gallery !== null"
      size="sm"
      color="neutral"
      variant="outline"
      class="shrink-0"
      data-testid="sitemap-gallery"
      @click="gallery"
    >
      {{ t('shell.sitemap.gallery') }}
    </UButton>
    <UButton size="sm" color="neutral" variant="ghost" class="ml-auto shrink-0" data-testid="sitemap-close" @click="close">
      {{ t('shell.sitemap.close') }}
    </UButton>
  </div>
</template>
