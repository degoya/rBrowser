<script setup lang="ts">
// The screenshot editor panel (RB-080-01, RB-080-63), on Nuxt UI since RB-080-86: opens a PNG of
// `.rbrowser/out/`, draws the nine tools on a canvas, keeps undo and redo, zooms, saves through
// the tool `annotate` (`shot_annotate`) and copies the edited image to the clipboard. The header
// holds the image actions (undo, redo, discard, copy, Save); the image to open, the tools with
// the chosen tool's options, the canvas with the crop's Apply and Cancel at its frame, the zoom
// and the annotations follow. The canvas is the panel's own (session.ts, draw.ts, input.ts,
// crop.ts). The plugin's events - `{ open }` and `{ copy: true }` - wait for the texts.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import AnnotationList from './AnnotationList.vue';
import { font } from './draw';
import FileBar from './FileBar.vue';
import { keyHandler } from './keys';
import { createSession } from './session';
import ShortcutButton from './ShortcutButton.vue';
import ToolBar from './ToolBar.vue';

const panel = usePanel();
const t = panel.t;
const session = createSession(panel);
const state = session.state;

const canvas = ref<HTMLCanvasElement | null>(null);
const frame = ref<HTMLElement | null>(null);
const stage = ref<HTMLElement | null>(null);
const cropBar = ref<HTMLElement | null>(null);
const textInput = ref<{ inputRef?: HTMLInputElement | null } | null>(null);

/** The status line: the session's sentence; the host's waiting unless a failure is shown. */
const line = computed<StatusLine | null>(() => {
  if (panel.waiting.value && state.status?.tone !== 'error') return { tone: 'busy', text: panel.strings.value['plugins.panel.waiting'] ?? '' };
  return state.status;
});
const zoomText = computed(() => t('panel.zoom', { percent: Math.round(state.zoom * 100) }));
const canvasLabel = computed(() =>
  state.image ? t('panel.canvas', { image: state.image.path, count: state.items.length }) : t('panel.canvas_empty'),
);
const noImage = computed(() => state.image === null);

const opens = (posted: unknown): posted is { open: unknown } => posted !== null && typeof posted === 'object' && 'open' in posted;
const early: unknown[] = [];

/** An event of the plugin, once the texts are there. */
function act(posted: unknown): void {
  if (!posted || typeof posted !== 'object') return;
  if (opens(posted)) {
    void session.refreshList();
    session.open(typeof posted.open === 'string' ? posted.open : null);
  } else if ((posted as { copy?: unknown }).copy === true) void session.copy();
}

/** The list and one image once the texts are there: the one an early `open` asked for, else the
 * newest (review of RB-080-59: two loads raced); then the other early events. */
function start(): void {
  const queued = early.splice(0);
  const asked = queued.filter(opens).at(-1);
  void session.refreshList();
  if (asked) session.open(typeof asked.open === 'string' ? asked.open : null);
  else void session.load(null);
  queued.filter((posted) => !opens(posted)).forEach(act);
}

// The plugin's posts; the host's own word (`waiting`) is panel.ts's.
const stopEvents = onPanelEvent((message) => {
  if (!message || typeof message !== 'object' || 'host' in message) return;
  if (panel.ready.value) act(message);
  else early.push(message);
});

panel.onSettings((first) => {
  session.applySettings(panel.settings.value);
  if (first) start();
});

const onKey = keyHandler(session, () => canvas.value);

// The panel renders once its texts are there (never a raw key); then the canvas is wired.
const stopAttach = watch(
  [canvas, frame, stage, cropBar],
  ([drawn, holder, scroller, bar]) => {
    if (!drawn || !holder || !scroller || !bar) return;
    session.attach({ canvas: drawn, frame: holder, stage: scroller, cropBar: bar });
    stopAttach();
  },
  { flush: 'post' },
);

onMounted(() => {
  document.addEventListener('keydown', onKey);
  if (document.fonts && typeof document.fonts.load === 'function') {
    document.fonts.load(font(20)).then(session.fontLoaded, () => undefined);
  }
});

onBeforeUnmount(() => {
  stopEvents();
  document.removeEventListener('keydown', onKey);
});

watch(
  () => state.textOpen,
  async (open) => {
    if (!open) return;
    await nextTick();
    textInput.value?.inputRef?.focus();
  },
);

function textKey(event: KeyboardEvent): void {
  if (event.key === 'Enter') {
    event.preventDefault();
    session.addText();
  } else if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    session.closeText();
  }
}
</script>

<template>
  <PanelFrame v-if="panel.ready.value" :title="t('panels.editor')" icon="i-lucide-camera" :status="line">
    <template #actions>
      <div class="flex items-center gap-0.5" role="group" :aria-label="t('panel.group_history')">
        <ShortcutButton id="undo" icon="i-lucide-undo-2" :label="t('panel.undo')" :keys="['Ctrl', 'Z']" shortcut="Control+Z" :disabled="!state.canUndo" @click="session.undo()" />
        <ShortcutButton id="redo" icon="i-lucide-redo-2" :label="t('panel.redo')" :keys="['Ctrl', 'Y']" shortcut="Control+Y" :disabled="!state.canRedo" @click="session.redo()" />
      </div>
      <USeparator orientation="vertical" class="h-5" />
      <div class="flex items-center gap-0.5" role="group" :aria-label="t('panel.group_output')">
        <ShortcutButton id="revert" icon="i-lucide-rotate-ccw" :label="t('panel.revert')" :keys="['Esc']" shortcut="Escape" :disabled="noImage" @click="session.revert()" />
        <ShortcutButton id="copy" icon="i-lucide-copy" :label="t('panel.copy')" :keys="['Ctrl', 'C']" shortcut="Control+C" :disabled="noImage" @click="session.copy()" />
        <ShortcutButton id="save" icon="i-lucide-save" :label="t('panel.save')" :keys="['Ctrl', 'S']" shortcut="Control+S" primary :disabled="noImage" @click="session.save()" />
      </div>
    </template>

    <div class="flex flex-col gap-3">
      <FileBar :session="session" />
      <ToolBar :session="session" />

      <div v-if="state.textOpen" id="text-entry" class="flex flex-wrap items-center gap-2">
        <label for="text-input" class="text-sm">{{ t('panel.text') }}</label>
        <UInput id="text-input" ref="textInput" v-model="state.text" size="sm" class="min-w-0 max-w-96 flex-1" maxlength="200" @keydown="textKey" />
        <UButton id="text-ok" :label="t('panel.text_ok')" size="sm" @click="session.addText()" />
        <UButton id="text-cancel" :label="t('panel.text_cancel')" color="neutral" variant="outline" size="sm" @click="session.closeText()" />
      </div>

      <div id="stage" ref="stage" class="stage relative max-h-[70vh] overflow-auto rounded-md border border-default bg-elevated p-0.5">
        <div id="frame" ref="frame" class="frame">
          <canvas id="canvas" ref="canvas" tabindex="0" role="img" aria-describedby="canvas-help" :aria-label="canvasLabel" width="1" height="1" />
        </div>
        <div
          id="crop-bar"
          ref="cropBar"
          class="absolute flex items-center gap-2 rounded-md border border-default bg-default px-2 py-1 shadow-md"
          role="group"
          :aria-label="t('panel.crop')"
          :hidden="state.crop === null"
        >
          <output id="crop-size" class="text-sm tabular-nums text-muted">{{ state.crop?.size }}</output>
          <ShortcutButton id="crop-apply" icon="i-lucide-check" :label="t('panel.crop_apply')" :keys="['Enter']" shortcut="Enter" primary :disabled="!state.crop?.applyable" @click="session.applyCrop()" />
          <ShortcutButton id="crop-cancel" icon="i-lucide-x" :label="t('panel.crop_cancel')" :keys="['Esc']" shortcut="Escape" text @click="session.cancelCrop()" />
        </div>
      </div>

      <div class="flex flex-wrap items-start gap-2">
        <p id="canvas-help" class="min-w-64 flex-1 text-xs text-muted">{{ t('panel.canvas_help') }}</p>
        <div class="flex shrink-0 items-center gap-0.5" role="group" :aria-label="t('panel.group_zoom')">
          <ShortcutButton id="zoom-out" icon="i-lucide-zoom-out" :label="t('panel.zoom_out')" :keys="['-']" shortcut="-" @click="session.stepZoom(-1)" />
          <output id="zoom-value" class="w-14 text-center text-sm tabular-nums">{{ zoomText }}</output>
          <ShortcutButton id="zoom-in" icon="i-lucide-zoom-in" :label="t('panel.zoom_in')" :keys="['+']" shortcut="+" @click="session.stepZoom(1)" />
          <ShortcutButton id="zoom-fit" icon="i-lucide-scan" :label="t('panel.zoom_fit')" :keys="['0']" shortcut="0" @click="session.fit()" />
        </div>
      </div>

      <AnnotationList :session="session" />
    </div>
  </PanelFrame>
</template>
