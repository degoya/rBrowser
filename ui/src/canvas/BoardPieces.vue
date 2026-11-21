<script setup lang="ts">
// The pieces of a canvas board label beside the name (RB-090-69): what a grid strip shows there
// (RB-090-27) - the address, and the external engine's window while the engine badge does not
// name it (RB-090-71) - worded by the core from the same data and the same `ui.cellLabel`
// settings. The Web Vitals are the vitals bar's, the native strip under the view (RB-090-71).
// Each is a button with its details as the tooltip and the accessible name (the native `title`:
// the board lies behind the views' overlays, so a tooltip it drew could be hidden behind them).
// A click, Enter or Space names the piece (`pick`); the core does what a click on the strip's
// piece does. Its press, release and keys never reach the label (no drag, no step). While the
// core has the address field of a decoupled view open (`edit`, RB-092-03), the field stands in
// the address's place (`BoardAddressField.vue`).
import { computed } from 'vue';
import UButton from '@nuxt/ui/components/Button.vue';
import type { LabelPiece } from '../ipc/layout';
import type { ViewId } from '../ipc/protocol';
import BoardAddressField from './BoardAddressField.vue';
import type { BoardEntryPiece } from './board';

const props = defineProps<{
  view: ViewId;
  /** The label is the active view's: the pieces take its inverted colours. */
  active: boolean;
  pieces: BoardEntryPiece[];
  /** The open address field: the address it starts with and the view's label. */
  edit?: { url: string; label: string } | null;
}>();

const shown = computed(() => (props.edit ? props.pieces.filter((piece) => piece.kind !== 'address') : props.pieces));

const emit = defineEmits<{ pick: [piece: LabelPiece] }>();
</script>

<template>
  <div class="contents">
    <BoardAddressField v-if="edit" :view="view" :url="edit.url" :label="edit.label" />
    <UButton
      v-for="piece in shown"
      :key="piece.kind"
      type="button"
      color="neutral"
      variant="ghost"
      size="xs"
      :label="piece.text"
      class="h-auto self-stretch rounded-sm px-1 py-0 text-xs text-inherit tabular-nums focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-(--ui-primary)"
      :class="[active ? 'bg-inverted/15' : 'bg-accented', piece.kind === 'address' ? 'min-w-0 shrink' : 'shrink-0']"
      :title="piece.tooltip"
      :aria-label="piece.tooltip"
      :data-testid="`board-piece-${piece.kind}-${view}`"
      @pointerdown.stop
      @pointerup.stop
      @keydown.stop
      @click="emit('pick', piece.kind)"
    />
  </div>
</template>
