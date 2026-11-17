<script setup lang="ts">
// A colour setting (RB-090-58), for every key of the colour kind (`format: "color"` or a name
// ending in colour), core and plugins alike: a swatch button opens `UColorPicker` in a popover,
// with an opacity slider where the setting takes `#rrggbbaa` (its `pattern`); the hex field
// beside it is the keyboard's way and takes any value typed. What the popover chose is committed
// once, when it closes - not on every move of the picker.
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UColorPicker from '@nuxt/ui/components/ColorPicker.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import UPopover from '@nuxt/ui/components/Popover.vue';
import USlider from '@nuxt/ui/components/Slider.vue';
import { opacityOf, parseColor, withOpacity, withRgb } from './colorValue';

const props = defineProps<{
  value: string;
  /** The accessible name. */
  label: string;
  inputId: string;
  describedBy?: string;
  disabled?: boolean;
  /** The setting takes an opacity (`#rrggbbaa`). */
  alpha: boolean;
}>();
const emit = defineEmits<{ update: [value: string] }>();
const { t } = useI18n();

/** What the popover shows: the value, then each move of the picker or the slider. */
const draft = ref(props.value);
watch(
  () => props.value,
  (now) => {
    draft.value = now;
  },
);
const hue = computed(() => parseColor(draft.value)?.rgb ?? '#000000');
const opacity = computed(() => opacityOf(draft.value));
/** The swatch: the colour over a checkerboard, so its opacity shows. */
const swatch = computed(() => {
  const color = parseColor(draft.value) ? draft.value : 'transparent';
  return {
    background: `linear-gradient(${color}, ${color}), repeating-conic-gradient(var(--ui-bg-accented) 0 25%, var(--ui-bg) 0 50%) 0 0 / 8px 8px`,
  };
});

function onPick(rgb: string | undefined): void {
  if (rgb !== undefined) draft.value = withRgb(draft.value, rgb, props.alpha);
}

function onOpacity(value: number | number[] | undefined): void {
  const percent = Array.isArray(value) ? value[0] : value;
  if (typeof percent === 'number') draft.value = withOpacity(draft.value, percent);
}

function onOpen(open: boolean): void {
  if (!open && draft.value !== props.value) emit('update', draft.value);
}

function onText(event: Event): void {
  emit('update', (event.target as HTMLInputElement).value);
}
</script>

<template>
  <div class="flex items-center gap-2">
    <UPopover :content="{ align: 'start' }" @update:open="onOpen">
      <UButton
        color="neutral"
        variant="outline"
        square
        :disabled="disabled"
        :aria-label="t('settings.color.pick', { name: label })"
        data-testid="color-pick"
      >
        <span class="block size-5 rounded-sm ring ring-inset ring-accented" :style="swatch" />
      </UButton>
      <template #content>
        <div class="flex w-56 flex-col gap-3 p-3" data-testid="color-popover">
          <UColorPicker :model-value="hue" size="sm" @update:model-value="onPick" />
          <USlider
            v-if="alpha"
            :model-value="opacity"
            :min="0"
            :max="100"
            size="sm"
            :aria-label="t('settings.color.opacity', { name: label })"
            data-testid="color-opacity"
            @update:model-value="onOpacity"
          />
        </div>
      </template>
    </UPopover>
    <UInput
      :id="inputId"
      :aria-describedby="describedBy"
      :model-value="value"
      :disabled="disabled"
      :aria-label="label"
      spellcheck="false"
      class="max-w-36"
      :ui="{ base: 'font-mono' }"
      @change="onText"
    />
  </div>
</template>
