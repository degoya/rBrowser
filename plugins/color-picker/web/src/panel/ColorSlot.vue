<script setup lang="ts">
// One colour of the contrast (RB-070-10, on Nuxt UI since RB-080-86): its swatch, the field that
// takes a colour in any CSS format (applied on change, as before), and every format to copy, the
// default format first. The swatch is our own - Nuxt UI has no colour box - a bordered square in
// the colour, hidden from readers (the formats say the colour).
import { ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import type { Format, Formats, Slot } from './model';

const props = defineProps<{ which: Slot; value: string; formats: Formats | null; order: Format[] }>();
const emit = defineEmits<{ change: [value: string]; copy: [value: string, field: HTMLInputElement | null] }>();
const { t } = usePanel();

const draft = ref(props.value);
const editing = ref(false);
// A colour picked or swapped shows in the field, unless the person is typing in it.
watch(
  () => props.value,
  (now) => {
    if (!editing.value) draft.value = now;
  },
);

function change(): void {
  const value = draft.value.trim();
  if (value) emit('change', value);
}

const fieldId = (format: Format): string => `${props.which}-${format}`;
const label = (format: Format): string => t(`panel.format.${format}`);

/** Copies a format's value; the field is the fallback's selection when the clipboard is shut. */
function copy(format: Format): void {
  if (props.formats) emit('copy', props.formats[format], document.getElementById(fieldId(format)) as HTMLInputElement | null);
}
</script>

<template>
  <fieldset :data-slot="which" class="flex min-w-0 flex-col gap-2 rounded-md border border-default p-2">
    <legend class="flex items-center gap-2 px-1 text-sm font-semibold text-highlighted">
      <span
        data-swatch
        aria-hidden="true"
        class="inline-block size-5 shrink-0 rounded border border-accented"
        :style="{ background: formats?.hex ?? 'transparent' }"
      />
      <span>{{ t(`panel.${which}`) }}</span>
    </legend>
    <UFormField :label="t('panel.edit')" :name="`${which}-input`" size="sm">
      <UInput
        :id="`${which}-input`"
        v-model="draft"
        class="w-full"
        spellcheck="false"
        autocomplete="off"
        @focus="editing = true"
        @blur="editing = false"
        @change="change"
      />
    </UFormField>
    <ul v-if="formats" data-formats class="flex flex-col gap-1">
      <li v-for="format in order" :key="format" :data-format="format" class="flex items-center gap-1.5">
        <span class="w-12 shrink-0 text-xs font-medium text-muted">{{ label(format) }}</span>
        <UInput
          :id="fieldId(format)"
          :model-value="formats[format]"
          readonly
          size="sm"
          variant="subtle"
          class="min-w-0 flex-1"
          :ui="{ base: 'font-mono' }"
          :aria-label="`${t(`panel.${which}`)}: ${label(format)}`"
        />
        <IconAction
          data-copy
          icon="i-lucide-copy"
          :label="t('panel.copy_format', { format: label(format), value: formats[format] })"
          @click="copy(format)"
        />
      </li>
    </ul>
  </fieldset>
</template>
