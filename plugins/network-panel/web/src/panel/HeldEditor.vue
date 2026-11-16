<script setup lang="ts">
// The held request's editor (RB-080-37, on Nuxt UI since RB-080-84): its URL (a request) or
// status (a response), headers and body in syntax colours (headers as HTTP, the body in the
// syntax its content type names), filled once per held request so a poll keeps the edits; then
// Continue (with the edits), Continue for all views, Abort, Abort for all views.
import { computed, reactive, watch } from 'vue';
import CodeEditor from '../../../../web-common/panel-ui/src/code/CodeEditor.vue';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import { contentType, heldEdits, heldFields, shortUrl, type HeldFields } from './model';
import { useNetwork } from './store';

const net = useNetwork();
const debug = net.debug;
const t = net.panel.t;
const current = computed(() => (debug.answer.value?.held ?? []).find((entry) => entry.id === debug.selected.value));
const fields = reactive<HeldFields>({ url: '', status: '', headers: '', body: '' });

watch(
  () => current.value?.id,
  () => {
    if (current.value) Object.assign(fields, heldFields(current.value));
  },
  { immediate: true },
);

const bodyLang = (text: string) => highlighter().detect(text, contentType(fields.headers));

function resolve(tool: 'continue' | 'abort', all: boolean): void {
  if (!current.value) return;
  debug.resolve(tool, all, tool === 'continue' ? heldEdits(current.value, fields) : undefined);
}
</script>

<template>
  <UCard v-if="current" id="held-editor" as="section" aria-labelledby="held-title" :ui="{ header: 'px-3 py-2 sm:px-3', body: 'flex flex-col gap-2 p-3 sm:p-3' }">
    <template #header>
      <h3 id="held-title" class="text-sm font-semibold text-highlighted">{{ current.method + ' ' + shortUrl(current.url) }}</h3>
    </template>
    <UFormField v-if="current.stage !== 'response'" id="held-url-row" :label="t('breakpoints.url')">
      <UInput id="held-url" v-model="fields.url" size="sm" :maxlength="2048" :spellcheck="false" class="w-full" />
    </UFormField>
    <UFormField v-else id="held-status-row" :label="t('breakpoints.status')">
      <UInput id="held-status" v-model="fields.status" type="number" :min="100" :max="599" size="sm" class="w-28" />
    </UFormField>
    <UFormField :label="t('breakpoints.headers')">
      <CodeEditor id="held-headers" v-model="fields.headers" lang="http" :rows="4" :label="t('breakpoints.headers')" />
    </UFormField>
    <UFormField :label="t('breakpoints.body')">
      <CodeEditor id="held-body" v-model="fields.body" :lang="bodyLang" :note="t('detail.plainLarge')" :rows="4" :disabled="Boolean(current.bodyBase64)" :label="t('breakpoints.body')" />
    </UFormField>
    <p id="held-body-note" class="text-xs text-muted">{{ current.bodyTruncated ? t('breakpoints.bodyCut') : t('breakpoints.masked') }}</p>
    <div class="flex flex-wrap gap-1.5">
      <UButton id="held-continue" size="sm" icon="i-lucide-play" :label="t('breakpoints.continue')" @click="resolve('continue', false)" />
      <UButton id="held-continue-all" size="sm" variant="outline" color="neutral" :label="t('breakpoints.continueAll')" @click="resolve('continue', true)" />
      <UButton id="held-abort" size="sm" variant="outline" color="error" icon="i-lucide-octagon-x" :label="t('breakpoints.abort')" @click="resolve('abort', false)" />
      <UButton id="held-abort-all" size="sm" variant="outline" color="error" :label="t('breakpoints.abortAll')" @click="resolve('abort', true)" />
    </div>
  </UCard>
</template>
