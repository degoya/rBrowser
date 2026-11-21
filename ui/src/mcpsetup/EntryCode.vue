<script setup lang="ts">
// The `rbrowser` entry as it is written (RB-091-09): JSON in syntax colours, tokenised like the
// settings' JSON view and escaped as text by Vue; Codex's TOML in one colour. Focusable, so the
// keyboard scrolls a long entry.
import { computed } from 'vue';
import { JSON_TOKEN_CLASSES, tokenizeJsonLines } from '../settings/jsonTokens';

const props = defineProps<{ code: string; label: string }>();
const json = computed(() => props.code.trimStart().startsWith('{'));
const lines = computed(() => (json.value ? tokenizeJsonLines(props.code) : []));
</script>

<template>
  <pre
    class="max-h-40 overflow-auto rounded bg-elevated p-2 font-mono text-xs text-default"
    tabindex="0"
    :aria-label="label"
  ><template v-if="json"><template v-for="(line, row) in lines" :key="row"><template v-for="(token, column) in line" :key="column"><span v-if="token.kind !== 'space'" :class="JSON_TOKEN_CLASSES[token.kind]">{{ token.text }}</span><template v-else>{{ token.text }}</template></template>{{ row < lines.length - 1 ? '\n' : '' }}</template></template><template v-else>{{ code }}</template></pre>
</template>
