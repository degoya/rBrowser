<script setup lang="ts">
// The command line's results (RB-060-02, on Nuxt UI since RB-080-84): per command its
// expression in syntax colours, then a line per distinct value with the dots of the views that
// gave it - a value in its language (a string as text, a function as JavaScript, JSON as JSON),
// a failure as a sentence when it is a code, else in the page's own words.
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type ExecRecord, type ViewEntry, isCode } from './model';
import Tokens from './Tokens.vue';
import ViewDots from './ViewDots.vue';

defineProps<{ records: ExecRecord[]; views: ViewEntry[] }>();
const panel = usePanel();
const t = panel.t;

const failure = (text: string): string => t('panel.resultError', { text: isCode(text) ? panel.error(text) : text });
</script>

<template>
  <ol id="results" aria-live="polite" :aria-label="t('panel.result')" class="flex max-h-48 flex-col gap-1.5 overflow-auto empty:hidden">
    <li v-for="record in records" :key="record.id" class="result rounded-md border border-default px-2 py-1.5 text-xs">
      <code class="expression block font-mono break-words whitespace-pre-wrap text-highlighted">&gt; <Tokens :text="record.expression" lang="js" /></code>
      <div v-if="record.error" class="outcome failed mt-1 flex items-center gap-1.5 text-error" data-tone="error">
        <UIcon name="i-lucide-circle-x" class="size-3.5 shrink-0" />{{ record.error }}
      </div>
      <div
        v-for="(result, index) in record.results"
        :key="index"
        class="outcome mt-1 flex items-start gap-1.5"
        :class="result.ok ? '' : 'failed text-error'"
        :data-tone="result.ok ? undefined : 'error'"
      >
        <ViewDots class="mt-1" :ids="result.views" :views="views" />
        <Tokens v-if="result.ok" class="min-w-0 font-mono break-words whitespace-pre-wrap" :text="result.text" :lang="highlighter().valueLang(result.type, result.text)" />
        <span v-else class="min-w-0 break-words">{{ failure(result.text) }}</span>
      </div>
    </li>
  </ol>
</template>
