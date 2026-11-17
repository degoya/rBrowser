<script setup lang="ts">
// A code view in a Nuxt UI panel (RB-080-84, RB-080-65): `text` coloured as `lang` by the one
// highlighter (highlight.ts beside it) - text nodes and spans only, the text is foreign -, or plain
// when `ui.syntaxHighlight` is off, there is no language or the text is too large (then `note`
// says so above it). The panel's settings switch the highlighter. A panel that uses it names
// this folder in its stylesheet (`@source '.../panel-ui/src/code';`), so its classes are built.
import { onMounted, ref, watch } from 'vue';
import { highlighter } from './highlight';
import { usePanel } from '../mount';

const props = defineProps<{ text: string; lang: HighlightLang | null; note?: string }>();
const { settings } = usePanel();
const pre = ref<HTMLElement | null>(null);

function paint(): void {
  if (pre.value) highlighter().render(pre.value, props.text, props.lang, props.note);
}

watch(
  settings,
  (now) => {
    highlighter().configure(now);
    paint();
  },
  { immediate: true },
);
watch(() => [props.text, props.lang, props.note], paint);
onMounted(paint);
</script>

<template>
  <pre
    ref="pre"
    tabindex="0"
    class="m-0 max-h-96 overflow-auto rounded-md border border-default bg-elevated/50 p-2 font-mono text-xs whitespace-pre-wrap break-all outline-none focus-visible:ring-2 focus-visible:ring-primary"
  />
</template>
