<script setup lang="ts">
// "Copy as" (RB-096-03): the request in the editor as curl, HTTPie, fetch or a Playwright test,
// written by the core (op `code`) for the shell the settings name, the plain values of the
// environment in, every secret a variable the code reads - the names shown, never a value. The
// header starts it (`RequestHeader.vue`); here, below the tabs, the code is shown and said
// copied - when the frame may not reach the clipboard, it stays on screen to select.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import CodeBlock from '../../../../web-common/panel-ui/src/code/CodeBlock.vue';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import { highlightOf } from './code';
import type { CodeRun } from './runs';

const props = defineProps<{ run: CodeRun }>();
const t = usePanel().t;

const languageName = computed(() => (props.run.snippet ? t(`panel.code.languages.${props.run.snippet.language}`) : ''));
</script>

<template>
  <p v-if="run.failure" id="code-failure" role="alert" class="text-xs text-error">{{ run.failure }}</p>
  <section v-if="run.snippet" id="code" class="flex flex-col gap-1" :aria-label="t('panel.code.title')">
    <div class="flex items-center gap-2">
      <p id="code-status" role="status" class="min-w-0 flex-1 text-xs" :class="run.copied ? 'text-success' : 'text-warning'">
        {{ run.copied ? t('panel.code.copied', { language: languageName }) : t('panel.code.notCopied') }}
      </p>
      <IconAction id="close-code" icon="i-lucide-x" :label="t('panel.code.close')" @click="run.snippet = null" />
    </div>
    <p v-if="run.snippet.env.length > 0" id="code-env" class="text-xs text-muted">{{ t('panel.code.env', { names: run.snippet.env.join(', ') }) }}</p>
    <ul v-if="run.snippet.notes.length > 0" id="code-notes" class="list-disc ps-4 text-xs text-warning">
      <li v-for="note in run.snippet.notes" :key="note">{{ t(`panel.code.notes.${note}`) }}</li>
    </ul>
    <CodeBlock :text="run.snippet.text" :lang="highlightOf(run.snippet.language)" />
  </section>
</template>
