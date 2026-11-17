<script setup lang="ts">
// One colour of the overlay (RB-090-58): a swatch button opens `UColorPicker` and an opacity
// slider in a popover - applied once, when it closes -, the hex field beside it is the keyboard's
// way (`#rrggbb` or `#rrggbbaa`, applied on change; anything else is marked and not applied).
// The texts come from `ColorSetup`, so the field needs no panel of its own.
import { computed, ref, watch } from 'vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UColorPicker from '@nuxt/ui/components/ColorPicker.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import UPopover from '@nuxt/ui/components/Popover.vue';
import USlider from '@nuxt/ui/components/Slider.vue';
import { hueOf, isColor, opacityOf, withOpacity, withRgb, type ColorKey } from './colors';

const props = defineProps<{
  value: string;
  colorKey: ColorKey;
  name: string;
  pickLabel: string;
  hexLabel: string;
  opacityLabel: string;
  invalidText: string;
}>();
const emit = defineEmits<{ update: [value: string] }>();

/** What the popover shows: the value, then each move of the picker or the slider. */
const draft = ref(props.value);
const invalid = ref(false);
watch(
  () => props.value,
  (now) => {
    draft.value = now;
    invalid.value = false;
  },
);
const swatch = computed(() => {
  const color = isColor(draft.value) ? draft.value : 'transparent';
  return {
    background: `linear-gradient(${color}, ${color}), repeating-conic-gradient(var(--ui-bg-accented) 0 25%, var(--ui-bg) 0 50%) 0 0 / 8px 8px`,
  };
});

function onPick(rgb: string | undefined): void {
  if (rgb !== undefined) draft.value = withRgb(draft.value, rgb);
}

function onOpacity(value: number | number[] | undefined): void {
  const percent = Array.isArray(value) ? value[0] : value;
  if (typeof percent === 'number') draft.value = withOpacity(draft.value, percent);
}

function onOpen(open: boolean): void {
  if (!open && draft.value !== props.value && isColor(draft.value)) emit('update', draft.value);
}

function onText(event: Event): void {
  const text = (event.target as HTMLInputElement).value.trim();
  invalid.value = !isColor(text);
  if (!invalid.value && text !== props.value) emit('update', text);
}
</script>

<template>
  <div class="flex flex-col gap-1">
    <span :id="`color-${colorKey}-name`" class="text-sm text-default">{{ name }}</span>
    <div class="flex items-center gap-2">
      <UPopover :content="{ align: 'start' }" @update:open="onOpen">
        <UButton :id="`color-${colorKey}-pick`" color="neutral" variant="outline" size="sm" square :aria-label="pickLabel">
          <span class="block size-4 rounded-sm ring ring-inset ring-accented" :style="swatch" />
        </UButton>
        <template #content>
          <div class="flex w-56 flex-col gap-3 p-3">
            <UColorPicker :model-value="hueOf(draft)" size="sm" @update:model-value="onPick" />
            <USlider :model-value="opacityOf(draft)" :min="0" :max="100" size="sm" :aria-label="opacityLabel" @update:model-value="onOpacity" />
          </div>
        </template>
      </UPopover>
      <UInput
        :id="`color-${colorKey}`"
        :model-value="value"
        size="sm"
        spellcheck="false"
        class="w-32"
        :ui="{ base: 'font-mono' }"
        :aria-label="hexLabel"
        :aria-invalid="invalid ? 'true' : undefined"
        :aria-describedby="invalid ? `color-${colorKey}-invalid` : undefined"
        @change="onText"
      />
    </div>
    <p v-if="invalid" :id="`color-${colorKey}-invalid`" class="text-xs text-error">{{ invalidText }}</p>
  </div>
</template>
