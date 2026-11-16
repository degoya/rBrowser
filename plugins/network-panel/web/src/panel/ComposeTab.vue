<script setup lang="ts">
// The Compose tab (RB-080-37, on Nuxt UI since RB-080-84): a request of your own - method, URL,
// the view whose context (cookies, user agent) it is sent from, chosen from the grid's views or
// none, how many times, headers and body in syntax colours - and the results, newest first, each
// with its status line, response headers (HTTP) and body in the syntax its content type names.
import { computed, reactive, ref, watch } from 'vue';
import CodeBlock from '../../../../web-common/panel-ui/src/code/CodeBlock.vue';
import CodeEditor from '../../../../web-common/panel-ui/src/code/CodeEditor.vue';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import { contentType, headerLines } from './model';
import { useNetwork } from './store';

const net = useNetwork();
const debug = net.debug;
const t = net.panel.t;
const NONE = 'none';
const form = reactive({ method: 'GET', url: '', view: '', times: '1', headers: '', body: '' });
/** The view chosen: NONE, a view's id, or not chosen yet (then the first view). */
const chosen = ref<string | null>(null);

const viewItems = computed(() => [{ label: t('compose.noView'), value: NONE }, ...debug.views().map((view) => ({ label: view.label, value: String(view.id) }))]);
const view = computed({
  get: () => {
    const ids = debug.views().map((one) => String(one.id));
    if (chosen.value === NONE || (chosen.value !== null && ids.includes(chosen.value))) return chosen.value;
    return ids[0] ?? NONE;
  },
  set: (value: string) => {
    chosen.value = value;
  },
});
watch(view, (now) => {
  form.view = now === NONE ? '' : now;
}, { immediate: true });

const bodyLang = (text: string) => highlighter().detect(text, contentType(form.headers));
const results = computed(() =>
  [...(debug.answer.value?.composed ?? [])].reverse().map((result) => {
    const headers = headerLines(result.headers ?? []);
    const body = result.body ?? '';
    return { result, headers, body, lang: highlighter().detect(body, contentType(headers)) };
  }),
);
</script>

<template>
  <div id="panel-compose" class="flex flex-col gap-3">
    <UCard :ui="{ header: 'px-3 py-2 sm:px-3', body: 'flex flex-col gap-2 p-3 sm:p-3' }">
      <template #header>
        <h2 id="compose-title" class="text-sm font-semibold text-highlighted">{{ t('compose.title') }}</h2>
      </template>
      <div class="flex flex-wrap items-end gap-2">
        <UFormField :label="t('compose.method')">
          <UInput id="compose-method" v-model="form.method" size="sm" :maxlength="16" :spellcheck="false" class="w-24" />
        </UFormField>
        <UFormField :label="t('compose.url')" class="min-w-48 flex-1">
          <UInput id="compose-url" v-model="form.url" type="url" size="sm" :maxlength="2048" :spellcheck="false" placeholder="https://" class="w-full" />
        </UFormField>
      </div>
      <div class="flex flex-wrap items-end gap-2">
        <UFormField :label="t('compose.view')">
          <USelect id="compose-view" v-model="view" :items="viewItems" size="sm" class="w-48" />
        </UFormField>
        <UFormField :label="t('compose.times')">
          <UInput id="compose-times" v-model="form.times" type="number" :min="1" :max="10" size="sm" class="w-20" />
        </UFormField>
      </div>
      <UFormField :label="t('compose.headers')">
        <CodeEditor id="compose-headers" v-model="form.headers" lang="http" :rows="3" :label="t('compose.headers')" />
      </UFormField>
      <UFormField :label="t('compose.body')">
        <CodeEditor id="compose-body" v-model="form.body" :lang="bodyLang" :note="t('detail.plainLarge')" :rows="3" :label="t('compose.body')" />
      </UFormField>
      <div>
        <UButton id="compose-send" size="sm" icon="i-lucide-send" :label="t('compose.send')" @click="debug.compose(form)" />
      </div>
    </UCard>

    <p v-if="debug.note.value" id="compose-note" role="status" class="text-sm text-muted">{{ debug.note.value }}</p>

    <section aria-labelledby="composed-title" class="flex flex-col gap-2">
      <h3 id="composed-title" class="text-xs font-semibold uppercase text-toned">{{ t('compose.results') }}</h3>
      <ol v-if="results.length" id="composed" class="flex flex-col gap-2">
        <li v-for="(one, index) in results" :key="index" data-composed class="flex flex-col gap-1 rounded-md border border-default p-2">
          <span data-line class="text-sm" :class="one.result.error ? 'text-error' : 'text-highlighted'">
            {{
              one.result.error
                ? t('compose.failed', { method: one.result.method, url: one.result.url, error: net.panel.error(one.result.error) })
                : t('compose.result', { method: one.result.method, url: one.result.url, status: one.result.status ?? '-', time: one.result.timeMs })
            }}
          </span>
          <CodeBlock v-if="one.headers" data-headers :text="one.headers" lang="http" />
          <p v-if="one.result.bodyBase64" data-binary class="text-sm text-muted">{{ t('detail.binary') }}</p>
          <CodeBlock v-else data-body :text="one.body" :lang="one.lang" :note="t('detail.plainLarge')" />
          <p v-if="!one.result.bodyBase64 && one.result.bodyTruncated" class="text-xs text-muted italic">{{ t('compose.cut') }}</p>
        </li>
      </ol>
      <p v-else id="composed-empty" class="text-sm text-muted">{{ t('compose.none') }}</p>
    </section>
  </div>
</template>
