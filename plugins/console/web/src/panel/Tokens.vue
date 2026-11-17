<script setup lang="ts">
// A piece of code inline (RB-080-84, RB-080-65): `text` cut by the one highlighter into spans of
// its colours and plain text nodes - text only, it is foreign -, plain when highlighting is off,
// there is no language or the text is too large.
import { computed } from 'vue';
import { markedTokens } from '../../../../web-common/panel-ui/src/code/highlight';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';

const props = defineProps<{ text: string; lang: HighlightLang | null }>();
const { settings } = usePanel();
const pieces = computed(() => {
  void settings.value;
  return markedTokens(props.text, props.lang, []);
});
</script>

<template>
  <span><template v-for="(piece, index) in pieces" :key="index"><span v-if="piece.kind !== 'plain'" :class="'hl-' + piece.kind">{{ piece.text }}</span><template v-else>{{ piece.text }}</template></template></span>
</template>
