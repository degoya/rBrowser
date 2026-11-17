<script setup lang="ts">
// The canvas minimap in the toolbar strip (RB-080-04): every view of the canvas and the part the
// grid area shows, in a reserved spot of the shell that never covers a view. A click centres the
// canvas on that point; dragging a view moves it there on release (snapped by the core); the
// arrow keys pan by a quarter of the visible area, Enter centres on the active view. Shown at
// the toolbar's start or end (`layoutModes.minimapPosition`) while `layoutModes.minimap` is on.
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import type { MinimapItem, MinimapPosition } from '../ipc/layout';
import { useShell } from '../state/shell';
import { panned, project, toCanvas, toMap } from './minimap';

const props = defineProps<{ position: MinimapPosition }>();

/** The minimap's size in px; the toolbar strip is 44 px tall. */
const WIDTH = 144;
const HEIGHT = 32;
/** The label band above each view, in canvas DIP (`LABEL_HEIGHT` of the core). */
const LABEL = 20;
/** A drag shorter than this many px is a click. */
const CLICK_SLOP = 3;

const shell = useShell();
const { t } = useI18n();

const map = computed(() => {
  const canvas = shell.state.value?.layout.modes?.canvas;
  return canvas && canvas.position === props.position ? canvas : null;
});
const projection = computed(() => (map.value ? project(map.value, WIDTH, HEIGHT) : null));
const shown = computed(() => {
  const p = projection.value;
  if (!map.value || !p) return null;
  return {
    items: map.value.items.map((item) => ({ item, rect: toMap(p, item.rect) })),
    viewport: toMap(p, map.value.viewport),
  };
});

interface Drag {
  item: MinimapItem;
  startX: number;
  startY: number;
  dx: number;
  dy: number;
}
const drag = ref<Drag | null>(null);

function local(event: PointerEvent): { x: number; y: number } {
  const box = (event.currentTarget as Element).getBoundingClientRect();
  return { x: event.clientX - box.left, y: event.clientY - box.top };
}

function onItemDown(item: MinimapItem, event: PointerEvent): void {
  event.stopPropagation();
  drag.value = { item, startX: event.clientX, startY: event.clientY, dx: 0, dy: 0 };
}

function onMove(event: PointerEvent): void {
  if (!drag.value) return;
  drag.value = { ...drag.value, dx: event.clientX - drag.value.startX, dy: event.clientY - drag.value.startY };
}

function onUp(event: PointerEvent): void {
  const p = projection.value;
  if (!p) return;
  const current = drag.value;
  drag.value = null;
  if (current && Math.hypot(current.dx, current.dy) >= CLICK_SLOP) {
    const x = Math.round(current.item.rect.x + current.dx / p.scale);
    const y = Math.round(current.item.rect.y + current.dy / p.scale + LABEL);
    void shell.run({ command: 'layout.canvas.place', args: { view: current.item.view, x, y } });
    return;
  }
  const point = local(event);
  const target = toCanvas(p, point.x, point.y);
  void shell.run({ command: 'layout.center', args: { x: Math.round(target.x), y: Math.round(target.y) } });
}

const PAN: Record<string, [number, number]> = {
  ArrowLeft: [-0.25, 0],
  ArrowRight: [0.25, 0],
  ArrowUp: [0, -0.25],
  ArrowDown: [0, 0.25],
};

function onKey(event: KeyboardEvent): void {
  if (!map.value) return;
  const step = PAN[event.key];
  if (step) {
    event.preventDefault();
    const target = panned(map.value.viewport, step[0], step[1]);
    void shell.run({ command: 'layout.center', args: { x: Math.round(target.x), y: Math.round(target.y) } });
  } else if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    void shell.run({ command: 'layout.center' });
  }
}
</script>

<template>
  <svg
    v-if="shown"
    :width="WIDTH"
    :height="HEIGHT"
    :viewBox="`0 0 ${WIDTH} ${HEIGHT}`"
    role="group"
    tabindex="0"
    :aria-label="t('shell.toolbar.minimap')"
    :aria-description="t('shell.toolbar.minimap_help')"
    class="shrink-0 cursor-crosshair touch-none rounded border border-default bg-elevated focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--ui-primary)"
    data-testid="minimap"
    @pointermove="onMove"
    @pointerup="onUp"
    @keydown="onKey"
  >
    <title>{{ t('shell.toolbar.minimap') }}</title>
    <rect
      v-for="{ item, rect } in shown.items"
      :key="item.view"
      :x="rect.x + (drag?.item.view === item.view ? drag.dx : 0)"
      :y="rect.y + (drag?.item.view === item.view ? drag.dy : 0)"
      :width="Math.max(1, rect.width)"
      :height="Math.max(1, rect.height)"
      :style="{
        fill: item.visible ? 'var(--ui-text-muted)' : 'var(--ui-text-dimmed)',
        stroke: 'var(--ui-text-highlighted)',
        strokeWidth: 0.5,
      }"
      class="cursor-move"
      :data-testid="`minimap-item-${item.view}`"
      @pointerdown="(event) => onItemDown(item, event)"
    >
      <title>{{ item.breakpoint }}</title>
    </rect>
    <rect
      :x="shown.viewport.x"
      :y="shown.viewport.y"
      :width="Math.max(1, shown.viewport.width)"
      :height="Math.max(1, shown.viewport.height)"
      :style="{ fill: 'none', stroke: 'var(--ui-primary)', strokeWidth: 1.5 }"
      pointer-events="none"
      data-testid="minimap-viewport"
    />
  </svg>
</template>
