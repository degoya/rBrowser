<script setup lang="ts">
// The sitemap bar (RB-050-05): `panels.sitemap` (`Ctrl+Shift+Space P G`, or the toolbar's
// indicator) reaches the shell as `uiCommand`, and this bar covers the toolbar strip (the shell
// view, never a page) with the last capture: its progress and time left while it runs, its
// outcome afterwards. Start, cancel, resume and "open the gallery" (in the active view) are
// native buttons, the first one focused; Escape closes. The coordinator mounts it in `App.vue`.
// Closed by Escape, Cancel or done, the focus goes back to its opener (`returnFocus.ts`, RB-090-23).
import { computed, nextTick, onScopeDispose, ref } from 'vue';
import { useI18n } from 'vue-i18n';
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
const view = computed(() => shell.state.value?.activeView ?? shell.state.value?.views[0]?.id ?? null);
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
  const path = status.value?.gallery;
  if (path !== undefined && view.value !== null) {
    void shell.send(galleryMessage(path, view.value));
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
    <button
      v-if="running"
      type="button"
      class="shrink-0 rounded-sm bg-error px-2 py-0.5 text-inverted focus-visible:outline-2 focus-visible:outline-primary"
      data-testid="sitemap-cancel"
      @click="cancel"
    >
      {{ t('shell.sitemap.cancel') }}
    </button>
    <template v-else>
      <button
        v-if="status?.state === 'cancelled'"
        type="button"
        class="shrink-0 rounded-sm border border-default px-2 py-0.5 focus-visible:outline-2 focus-visible:outline-primary"
        data-testid="sitemap-resume"
        @click="resume"
      >
        {{ t('shell.sitemap.resume') }}
      </button>
      <button
        type="button"
        class="shrink-0 rounded-sm border border-default px-2 py-0.5 focus-visible:outline-2 focus-visible:outline-primary"
        data-testid="sitemap-start"
        @click="start"
      >
        {{ t('shell.sitemap.start') }}
      </button>
    </template>
    <button
      v-if="status?.gallery !== undefined && status?.gallery !== null && view !== null"
      type="button"
      class="shrink-0 rounded-sm border border-default px-2 py-0.5 focus-visible:outline-2 focus-visible:outline-primary"
      data-testid="sitemap-gallery"
      @click="gallery"
    >
      {{ t('shell.sitemap.gallery') }}
    </button>
    <button
      type="button"
      class="ml-auto shrink-0 rounded-sm px-2 py-0.5 text-muted focus-visible:outline-2 focus-visible:outline-primary"
      data-testid="sitemap-close"
      @click="close"
    >
      {{ t('shell.sitemap.close') }}
    </button>
  </div>
</template>
