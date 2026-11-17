<script setup lang="ts">
// The overlay's own settings (RB-070-09, on Nuxt UI since RB-080-86): show it, then Appearance
// (opacity, blend mode, invert, position), Offset (X and Y, the arrow keys on Move: 1 px, Shift
// 10 px, reset) and Split view, one setting per row. Each change is one `change` with the
// arguments of the tool `set`; a slider sends its value when let go (or per key), and shows it
// while it moves.
import { computed, nextTick, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { ANCHORS, BLENDS, nudged, type OverlayState } from './model';

const props = defineProps<{ state: OverlayState }>();
const emit = defineEmits<{ change: [args: Record<string, unknown>] }>();
const { t } = usePanel();

/** The sliders' values while they move, back to the state's with every answer. */
const opacity = ref(props.state.opacity);
const position = ref(props.state.split.position);
/** The offset the next arrow key counts from, before the answer is back. */
let offset = { ...props.state.offset };

watch(
  () => props.state,
  (now) => {
    opacity.value = now.opacity;
    position.value = now.split.position;
    offset = { ...now.offset };
  },
);

const blends = computed(() => BLENDS.map((value) => ({ value, label: t(`panel.blend_${value}`) })));
const anchors = computed(() => ANCHORS.map((value) => ({ value, label: t(`panel.anchor_${value}`) })));

function nudge(event: KeyboardEvent): void {
  const next = nudged(offset, event.key, event.shiftKey);
  if (!next) return;
  event.preventDefault();
  offset = next;
  emit('change', { offset: next });
}

const number = (value: unknown): number => Number(value) || 0;

/** A slider's value when let go: its commit comes before its new value, so a tick later. */
function commit(args: () => Record<string, unknown>): void {
  void nextTick(() => emit('change', args()));
}
</script>

<template>
  <div class="flex flex-col gap-5">
    <USwitch
      id="enabled"
      :model-value="state.enabled"
      :label="t('panel.show')"
      @update:model-value="(on: boolean) => emit('change', { enabled: on })"
    />

    <section aria-labelledby="appearance-title" class="flex flex-col gap-3">
      <h2 id="appearance-title" class="text-sm font-semibold text-highlighted">{{ t('panel.appearance') }}</h2>
      <UFormField :label="t('panel.opacity')">
        <div class="flex items-center gap-3">
          <USlider id="opacity" v-model="opacity" :min="0" :max="100" :step="1" :aria-label="t('panel.opacity')" aria-describedby="opacity-value" class="flex-1" @change="commit(() => ({ opacity }))" />
          <output id="opacity-value" for="opacity" class="w-12 shrink-0 text-right text-sm tabular-nums text-muted">{{ t('panel.percent', { value: opacity }) }}</output>
        </div>
      </UFormField>
      <UFormField :label="t('panel.blend')">
        <USelect id="blend" :model-value="state.blend" :items="blends" size="sm" class="w-full max-w-60" @update:model-value="(value: string) => emit('change', { blend: value })" />
      </UFormField>
      <USwitch id="invert" :model-value="state.invert" :label="t('panel.invert')" @update:model-value="(on: boolean) => emit('change', { invert: on })" />
      <URadioGroup
        id="anchor"
        :legend="t('panel.anchor')"
        :model-value="state.anchor"
        :items="anchors"
        @update:model-value="(value: string) => emit('change', { anchor: value })"
      />
    </section>

    <section aria-labelledby="offset-title" class="flex flex-col gap-3">
      <h2 id="offset-title" class="text-sm font-semibold text-highlighted">{{ t('panel.offset') }}</h2>
      <!-- X and Y side by side: one offset in its two directions. -->
      <div class="grid grid-cols-2 gap-3">
        <UFormField :label="t('panel.offset_x')">
          <UInputNumber id="offset-x" :model-value="state.offset.x" :min="-100000" :max="100000" :step="1" size="sm" class="w-full" @update:model-value="(x: unknown) => emit('change', { offset: { x: number(x) } })" />
        </UFormField>
        <UFormField :label="t('panel.offset_y')">
          <UInputNumber id="offset-y" :model-value="state.offset.y" :min="-100000" :max="100000" :step="1" size="sm" class="w-full" @update:model-value="(y: unknown) => emit('change', { offset: { y: number(y) } })" />
        </UFormField>
      </div>
      <div class="flex flex-wrap gap-2">
        <UButton id="nudge" icon="i-lucide-move" color="primary" variant="outline" size="sm" :label="t('panel.nudge')" aria-describedby="nudge-hint" @keydown="nudge" />
        <UButton id="offset-reset" icon="i-lucide-locate-fixed" color="neutral" variant="outline" size="sm" :label="t('panel.offset_reset')" @click="emit('change', { offset: { x: 0, y: 0 } })" />
      </div>
      <p id="nudge-hint" class="text-xs text-muted">{{ t('panel.nudge_hint') }}</p>
    </section>

    <section aria-labelledby="split-title" class="flex flex-col gap-3">
      <h2 id="split-title" class="text-sm font-semibold text-highlighted">{{ t('panel.split') }}</h2>
      <USwitch id="split-on" :model-value="state.split.on" :label="t('panel.split_on')" @update:model-value="(on: boolean) => emit('change', { split: on })" />
      <UFormField :label="t('panel.split_position')">
        <div class="flex items-center gap-3">
          <USlider id="split-position" v-model="position" :min="0" :max="100" :step="1" :aria-label="t('panel.split_position')" aria-describedby="split-value" class="flex-1" @change="commit(() => ({ split: position }))" />
          <output id="split-value" for="split-position" class="w-12 shrink-0 text-right text-sm tabular-nums text-muted">{{ t('panel.percent', { value: position }) }}</output>
        </div>
      </UFormField>
    </section>
  </div>
</template>
