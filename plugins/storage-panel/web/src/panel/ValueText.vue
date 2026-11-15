<script setup lang="ts">
// A stored value in the grid (RB-080-65, RB-080-84): JSON in syntax colours by the one
// highlighter - text nodes and spans only, the value is the page's -, anything else plain; plain
// too with `ui.syntaxHighlight` off. The panel's settings repaint it.
import { onMounted, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import { jsonOf } from './model';

const props = defineProps<{ text: string }>();
const { settings } = usePanel();
const span = ref<HTMLElement | null>(null);

function paint(): void {
  const hl = highlighter();
  if (span.value) hl.render(span.value, props.text, jsonOf(props.text, (text) => hl.detect(text)));
}

watch(() => props.text, paint);
watch(settings, paint);
onMounted(paint);
</script>

<template>
  <span ref="span" class="value block min-w-0 truncate font-mono text-xs" />
</template>
