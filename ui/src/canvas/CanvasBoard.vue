<script setup lang="ts">
// The canvas board (RB-080-04, `canvas-board.html`): a CEF view of its own behind the canvas's
// views, the size of the grid area. It draws each view's label strip (the views' overlays leave
// the band above them free) and takes the drags the views' native panels cannot: a label dragged
// moves its view (live, snapped and kept on release), the empty board dragged pans the canvas,
// Escape cancels; with `layoutModes.spacePan` Space held turns a drag on a label into a pan. A
// label is focusable: the arrow keys move its view by the snap grid, Enter or Space makes it
// active; the label and its engine badge are two buttons side by side (RB-090-23). A
// click on a label makes its view active; a click on the active view's label asks the core for
// the size quick pick under it, as in the other modes (RB-080-22). Each label ends in the view's
// engine badge (RB-080-33, `ui.engineBadge`): a click on it asks the core for the engine menu
// under the label; a badge whose engine cannot run is dimmed and says why. A right-click on a
// label asks the core for the view's menu, one on the free area for the add menu (RB-080-41,
// `ui.viewContextMenu`): the board lies behind the views' native overlays, so a menu it drew
// itself lay behind them - it names the place (`canvasMenu`) and the core shows a native menu in
// front of the views (RB-080-46). The ContextMenu key on a focused label does the same.
// After the name come the pieces a grid strip shows (RB-090-69, `BoardPieces.vue`): the address
// and the external engine's window - the latter in the engine badge while it shows, one badge
// naming the engine and its window's state (RB-090-71); a click asks the core to do what the
// strip's piece does (`canvasPiece`: restart or raise the engine, activate). The Web Vitals are
// the vitals bar's, the native strip under the view (RB-090-71).
// A zoomed page shows its level between the label and the engine badge, as the grid's strips do
// (RB-090-28): its click, Enter or Space sets the page back to 100 % (`view.page_zoom.reset`).
// Ctrl+wheel zooms the canvas around the pointer (`canvasZoom`, `layoutModes.wheelZoom`): the
// core scales the views and the labels follow - the board's page itself never zooms (RB-080-50).
// The core moves the views only once the board painted their frames: the board answers each
// canvas it is sent (`canvasLive`) in the frame that paints it, for the newest one (RB-090-55).
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UIcon from '@nuxt/ui/components/Icon.vue';
import { ICONS } from '../icons';
import type { LabelPiece, MapRect } from '../ipc/layout';
import type { ViewId } from '../ipc/protocol';
import { useShell } from '../state/shell';
import BoardPieces from './BoardPieces.vue';
import {
  badgeState,
  badgeTitle,
  boardItems,
  createBoardDrag,
  labelPieces,
  offsetOf,
  stepFor,
  type BoardEntry,
  type DragState,
} from './board';

/** How long a let-go label keeps its drawn place while the core's new state is on its way. */
const SETTLE_MS = 300;

const shell = useShell();
const { t } = useI18n();

/** The state as the board draws it: with the frames of a camera a gesture is trying out. */
const drawn = computed(() => {
  const state = shell.state.value;
  const live = shell.liveBoard.value;
  if (state === null || live === null || state.layout.modes?.board === undefined) return state;
  return { ...state, layout: { ...state.layout, modes: { ...state.layout.modes, board: live } } };
});
const board = computed(() => drawn.value?.layout.modes?.board ?? null);
const items = computed(() => boardItems(drawn.value));
/** The drag under way, or the last one until the core's state shows its result. */
const live = ref<DragState | null>(null);
const space = ref(false);
/** A drag started while Space was held on a label: its release activates nothing. */
let spaceDragged = false;
let settleTimer: ReturnType<typeof setTimeout> | undefined;

const drag = createBoardDrag({
  send: (request) => void shell.send(request),
  frame: (callback) => requestAnimationFrame(callback),
  cancelFrame: (handle) => cancelAnimationFrame(handle),
});

const spacePans = computed(() => space.value && board.value?.spacePan === true);
/** `ui.engineBadge`: the badge shows unless the setting says no. */
const badges = computed(() => shell.settings.value['ui.engineBadge'] !== false);
/** `ui.viewContextMenu`: the menus open unless the setting says no. */
const menus = computed(() => shell.settings.value['ui.viewContextMenu'] !== false);
const dragging = computed(() => live.value?.moved === true);

// A right-click never starts a drag and never shows the page's own menu: it names its place on
// the board - with the label's view, or the free area without one - and the core opens the menu.
function onContextMenu(event: MouseEvent, view?: ViewId): void {
  event.preventDefault();
  // A label's click is not the free area's too.
  event.stopPropagation();
  if (!menus.value) return;
  void shell.send({
    type: 'canvasMenu',
    ...(view === undefined ? {} : { view }),
    x: Math.round(event.clientX),
    y: Math.round(event.clientY),
  });
}

/** How long the wheel rests before its zoom is kept (the core's `canvas_zoom::REST_MS`). */
const WHEEL_REST_MS = 150;
/**
 * The wheel's gesture: where it began, its whole turn in CSS px, whether a frame is waiting and
 * whether the core showed one.
 */
let wheel: { x: number; y: number; delta: number; queued: boolean; shown: boolean } | null = null;
let wheelRest: ReturnType<typeof setTimeout> | undefined;

// A turn back to where it began is still sent once a frame was shown: the views and the frames
// go back with it, and its end puts them back for good (RB-090-55).
function sendWheel(phase: 'move' | 'end'): void {
  if (wheel === null || (Math.round(wheel.delta) === 0 && !wheel.shown)) return;
  wheel.shown = true;
  void shell.send({ type: 'canvasZoom', x: wheel.x, y: wheel.y, delta: Math.round(wheel.delta), phase });
}

// Ctrl+wheel is never the page's zoom; with `layoutModes.wheelZoom` it is the canvas's. As a
// drag: the turn counts from where the gesture began, one frame at a time shows it, and the
// core keeps the zoom once the wheel has rested.
function onWheel(event: WheelEvent): void {
  if (!event.ctrlKey) return;
  event.preventDefault();
  if (board.value?.wheelZoom === false) return;
  wheel ??= { x: Math.round(event.clientX), y: Math.round(event.clientY), delta: 0, queued: false, shown: false };
  wheel.delta += event.deltaY;
  if (!wheel.queued) {
    wheel.queued = true;
    requestAnimationFrame(() => {
      if (wheel !== null) {
        wheel.queued = false;
        sendWheel('move');
      }
    });
  }
  clearTimeout(wheelRest);
  wheelRest = setTimeout(() => {
    sendWheel('end');
    wheel = null;
  }, WHEEL_REST_MS);
}

/** The newest canvas drawn and not yet answered for, and the frame that answers. */
let painted: number | null = null;
let paintFrame: number | undefined;

// The canvas `seq` is drawn (a post-flush watcher): the frame about to paint it answers, for the
// newest canvas only - the core then moves the views where their frames are.
watch(
  () => shell.liveSeq.value,
  (seq) => {
    if (seq === null) return;
    painted = seq;
    paintFrame ??= requestAnimationFrame(() => {
      paintFrame = undefined;
      if (painted !== null) void shell.send({ type: 'canvasPainted', seq: painted });
      painted = null;
    });
  },
  { flush: 'post' },
);

function settle(): void {
  clearTimeout(settleTimer);
  settleTimer = setTimeout(() => (live.value = null), SETTLE_MS);
}

// The core's new places arrived: draw them as they are.
watch(
  () => JSON.stringify(board.value?.items ?? []),
  () => {
    if (drag.current() === null) {
      clearTimeout(settleTimer);
      live.value = null;
    }
  },
);

function capture(event: PointerEvent): void {
  const target = event.currentTarget as Element | null;
  try {
    target?.setPointerCapture?.(event.pointerId);
  } catch {
    // A synthetic event has no pointer to capture; the board still hears the moves.
  }
}

function onLabelDown(view: ViewId, event: PointerEvent): void {
  if (event.button !== 0) return;
  event.stopPropagation();
  capture(event);
  if (space.value) spaceDragged = true;
  drag.start(spacePans.value ? { kind: 'pan' } : { kind: 'view', view }, event.clientX, event.clientY);
  live.value = drag.current();
}

function onBoardDown(event: PointerEvent): void {
  if (event.button !== 0) return;
  capture(event);
  drag.start({ kind: 'pan' }, event.clientX, event.clientY);
  live.value = drag.current();
}

function onMove(event: PointerEvent): void {
  if (drag.current() === null) return;
  // No button held: the release happened where the board did not hear it - the drag ends there.
  if (event.buttons === 0) {
    onUp(event);
    return;
  }
  live.value = drag.move(event.clientX, event.clientY);
}

function onUp(event: PointerEvent): void {
  const target = drag.current()?.target;
  const last = drag.move(event.clientX, event.clientY);
  const result = drag.end(event.clientX, event.clientY);
  if (result === 'click' && target?.kind === 'view') {
    live.value = null;
    const active = shell.state.value?.activeView === target.view;
    void shell.send(
      active ? { type: 'canvasSizePick', view: target.view } : { type: 'setActiveView', view: target.view },
    );
  } else if (result === 'dragged') {
    live.value = last;
    settle();
  } else {
    live.value = null;
  }
}

function onCancel(): void {
  if (drag.cancel()) live.value = null;
}

function onKeyDown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && drag.cancel()) {
    event.preventDefault();
    live.value = null;
  } else if (event.key === ' ') {
    // Space only arms the pan; it scrolls nothing here.
    event.preventDefault();
    space.value = true;
  }
}

function onKeyUp(event: KeyboardEvent): void {
  if (event.key === ' ') space.value = false;
}

// Losing the focus cancels no drag: moving the view's overlay may take the focus from the
// board (the window activates the overlay), and the pointer stays captured until let go.
function onBlur(): void {
  space.value = false;
}

function onLabelKey(view: ViewId, event: KeyboardEvent): void {
  const step = stepFor(event.key);
  if (step) {
    event.preventDefault();
    void shell.send({ type: 'canvasStep', view, ...step });
  } else if (event.key === 'Enter') {
    event.preventDefault();
    void shell.send({ type: 'setActiveView', view });
  } else if (event.key === ' ' && !event.repeat) {
    spaceDragged = false;
  }
}

// Space activates a focused label as Enter does, on its release like a native button - unless the
// Space held armed a pan that a drag used (RB-090-23).
function onLabelKeyUp(view: ViewId, event: KeyboardEvent): void {
  if (event.key !== ' ') return;
  event.preventDefault();
  if (!spaceDragged) {
    void shell.send({ type: 'setActiveView', view });
  }
  spaceDragged = false;
}

/** The badge's name: the engine, and why it cannot run - not only its dimmed look (RB-090-23);
 *  with its window's state when the badge names it (RB-090-71). */
function engineName(item: BoardEntry): string {
  const state = badgeState(item.pieces, badges.value);
  const engine = state?.text ?? item.engineLabel;
  return item.engineReason === null
    ? t('shell.board.engine', { label: item.label, engine })
    : t('shell.board.engineUnavailable', { label: item.label, engine, reason: item.engineReason });
}

/** The badge's tooltip: the window's state first when the badge names it, a dimmed badge's
 *  reason alone (RB-090-71). */
function engineTitle(item: BoardEntry): string {
  const own =
    item.engineReason === null
      ? t('engines.badge.tooltip', { engine: item.engineLabel })
      : t('engines.badge.unavailable', { engine: item.engineLabel, reason: item.engineReason });
  return badgeTitle(own, item.engineReason !== null, badgeState(item.pieces, badges.value));
}

// The zoom badge's click sets the view's page back to 100 % (RB-090-28).
function onZoomClick(view: ViewId): void {
  void shell.send({ type: 'runCommand', id: 'view.page_zoom.reset', args: { target: view } });
}

// A piece's click: the core looks up what it does on the grid strip and does it (RB-090-69).
function onPieceClick(view: ViewId, piece: LabelPiece): void {
  void shell.send({ type: 'canvasPiece', view, piece });
}

// The badge's click asks for the engine menu; the press never starts a drag of the label.
function onEngineClick(view: ViewId): void {
  void shell.send({ type: 'canvasEnginePick', view });
}

function box(rect: MapRect, view: ViewId): Record<string, string> {
  const offset = offsetOf(view, live.value);
  return {
    left: `${rect.x}px`,
    top: `${rect.y}px`,
    width: `${rect.width}px`,
    height: `${rect.height}px`,
    transform: `translate(${offset.x}px, ${offset.y}px)`,
  };
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown);
  window.addEventListener('keyup', onKeyUp);
  window.addEventListener('blur', onBlur);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown);
  window.removeEventListener('keyup', onKeyUp);
  window.removeEventListener('blur', onBlur);
  clearTimeout(settleTimer);
  clearTimeout(wheelRest);
  if (paintFrame !== undefined) cancelAnimationFrame(paintFrame);
});
</script>

<template>
  <main
    class="relative h-screen w-screen touch-none overflow-hidden bg-muted select-none"
    :class="dragging || spacePans ? 'cursor-grabbing' : 'cursor-grab'"
    role="group"
    :aria-label="t('shell.board.area')"
    data-testid="board"
    @pointerdown="onBoardDown"
    @pointermove="onMove"
    @pointerup="onUp"
    @pointercancel="onCancel"
    @contextmenu="(event) => onContextMenu(event)"
    @wheel="onWheel"
  >
    <template v-for="item in items" :key="item.view">
      <div
        class="pointer-events-none absolute rounded-sm border border-dashed border-accented"
        :style="box(item.rect, item.view)"
        aria-hidden="true"
        :data-testid="`board-frame-${item.view}`"
      />
      <div
        class="absolute flex items-stretch rounded-t-sm text-xs leading-5"
        :class="[
          item.active ? 'bg-primary text-inverted' : 'bg-elevated text-default',
          item.visible ? '' : 'opacity-70',
          spacePans ? 'cursor-grabbing' : 'cursor-move',
        ]"
        :style="box(item.labelRect, item.view)"
        :data-testid="`board-strip-${item.view}`"
        @pointerdown="(event) => onLabelDown(item.view, event)"
        @contextmenu="(event) => onContextMenu(event, item.view)"
      >
        <div
          role="button"
          tabindex="0"
          class="flex min-w-0 flex-1 items-center gap-1.5 truncate rounded-t-sm px-2 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-(--ui-primary)"
          :aria-label="item.visible ? item.label : t('shell.board.hidden', { label: item.label })"
          aria-describedby="board-hint"
          :aria-current="item.active ? 'true' : undefined"
          :data-testid="`board-label-${item.view}`"
          @keydown="(event) => onLabelKey(item.view, event)"
          @keyup="(event) => onLabelKeyUp(item.view, event)"
        >
          <span
            v-if="item.color"
            class="size-2 shrink-0 rounded-full"
            :style="{ background: item.color }"
            aria-hidden="true"
          />
          <span class="truncate">{{ item.label }}</span>
        </div>
        <BoardPieces
          :view="item.view"
          :active="item.active"
          :pieces="labelPieces(item.pieces, badges)"
          @pick="(piece) => onPieceClick(item.view, piece)"
        />
        <button
          v-if="item.pageZoom !== null"
          type="button"
          class="flex shrink-0 items-center self-stretch rounded-sm px-1 text-xs tabular-nums focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-(--ui-primary)"
          :class="item.active ? 'bg-inverted/15' : 'bg-accented'"
          :title="t('views.page_zoom.tooltip', { percent: item.pageZoom })"
          :aria-label="t('shell.board.pageZoom', { label: item.label, percent: item.pageZoom })"
          :data-testid="`board-zoom-${item.view}`"
          @pointerdown.stop
          @pointerup.stop
          @keydown.stop
          @click="onZoomClick(item.view)"
        >
          {{ t('views.page_zoom.badge', { percent: item.pageZoom }) }}
        </button>
        <button
          v-if="badges"
          type="button"
          class="flex min-w-6 shrink-0 items-center justify-center gap-0.5 self-stretch rounded-sm px-1 text-xs focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-(--ui-primary)"
          :class="[
            item.active ? 'bg-inverted/15' : 'bg-accented',
            item.engineReason === null ? '' : 'opacity-60',
          ]"
          :title="engineTitle(item)"
          :aria-label="engineName(item)"
          :data-available="item.engineReason === null ? 'true' : 'false'"
          :data-testid="`board-engine-${item.view}`"
          @pointerdown.stop
          @pointerup.stop
          @keydown.stop
          @click="onEngineClick(item.view)"
        >
          <UIcon
            v-if="item.engineReason !== null"
            :name="ICONS.statusWarning"
            class="size-3 shrink-0"
            aria-hidden="true"
            data-testid="board-engine-warning"
          />
          {{ badgeState(item.pieces, badges)?.text ?? item.engineLabel }}
        </button>
      </div>
    </template>
    <p id="board-hint" class="sr-only">{{ t('shell.board.hint') }} {{ t('shell.board.space') }} {{ t('shell.board.pick') }}</p>
  </main>
</template>
