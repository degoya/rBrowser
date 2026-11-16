<script setup lang="ts">
// The editor's value field (RB-080-65, on Nuxt UI since RB-080-84): a textarea coloured by what
// its text looks like through the one highlighter's `editor`, which lays a coloured copy under
// the textarea's own transparent text. Our own element: the highlighter wraps the textarea it
// colours, which a Nuxt UI UTextarea (its textarea Vue's) cannot give up; so the textarea is
// made here, outside Vue's tree, and styled like Nuxt UI's.
import { onMounted, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';

const props = defineProps<{ id: string }>();
const { t } = usePanel();
const host = ref<HTMLElement | null>(null);
let field: HTMLTextAreaElement | null = null;

onMounted(() => {
  if (!host.value) return;
  field = document.createElement('textarea');
  field.id = props.id;
  field.rows = 3;
  field.spellcheck = false;
  field.autocomplete = 'off';
  field.className =
    'w-full rounded-md border-0 bg-default px-2.5 py-1.5 font-mono text-xs text-highlighted ring ring-inset ring-accented focus-visible:ring-2 focus-visible:ring-primary outline-none';
  host.value.append(field);
  const hl = highlighter();
  hl.editor(field, (text) => hl.detect(text), () => t('panel.plainLarge'));
});

defineExpose({
  get: (): string => field?.value ?? '',
  set: (value: string): void => {
    if (field) field.value = value;
  },
  focus: (): void => field?.focus(),
});
</script>

<template>
  <div ref="host" />
</template>
