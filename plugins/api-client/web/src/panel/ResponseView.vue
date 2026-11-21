<script setup lang="ts">
// The answer (RB-096-01): status, total time and size in one row, the redirects it took, then
// the body - JSON as a tree or as source, XML and HTML as source, an image as itself, other bytes
// by their size -, the headers, the cookies it sets and the phases of the exchange. Every secret
// the core put in is masked already (`***`). Headers and phases are native `<dl>` lists: two
// columns of names and values, nothing to sort or select (docs/design.md 5.1).
import { computed, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import CodeBlock from '../../../../web-common/panel-ui/src/code/CodeBlock.vue';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import JsonTree from './JsonTree.vue';
import VerdictList from './VerdictList.vue';
import { cookiesOf } from './cookies';
import { type Answer, bodyView, bytes, millis, parsedBody, statusTone } from './model';

const props = defineProps<{ answer: Answer }>();
const t = usePanel().t;

const TABS = ['body', 'headers', 'cookies', 'timings'] as const;
const tab = ref<string>('body');
const asTree = ref(true);
const view = computed(() => bodyView(props.answer));
const json = computed(() => (view.value === 'json' ? parsedBody(props.answer) : undefined));
const cookies = computed(() => cookiesOf(props.answer.headers));
const tabs = computed(() =>
  TABS.map((name) => ({
    label: t(`panel.response.tabs.${name}`),
    value: name,
    slot: name,
    badge:
      name === 'headers' ? String(props.answer.headers.length) : name === 'cookies' && cookies.value.length > 0 ? String(cookies.value.length) : undefined,
  })),
);
const source = computed(() => {
  if (json.value !== undefined) return JSON.stringify(json.value, null, 2);
  return props.answer.body;
});
const lang = computed(() => (view.value === 'json' ? 'json' : view.value === 'xml' ? 'xml' : view.value === 'html' ? 'html' : null));
const image = computed(() => (view.value === 'image' && props.answer.bodyEncoding === 'base64' ? `data:${props.answer.contentType ?? 'image/png'};base64,${props.answer.body}` : ''));
const phases = computed(() => {
  const timings = props.answer.timings;
  return [
    ['dns', timings.dns],
    ['connect', timings.connect],
    ['tls', timings.tls],
    ['firstByte', timings.firstByte],
    ['total', timings.total],
  ] as const;
});

watch(
  () => props.answer,
  () => {
    asTree.value = true;
  },
);
</script>

<template>
  <section id="response" class="flex flex-col gap-2" :aria-label="t('panel.response.title')">
    <div id="summary" class="flex flex-wrap items-center gap-2 text-sm">
      <UBadge id="status-code" :color="statusTone(answer.status)" variant="subtle" size="md" :label="`${answer.status} ${answer.statusText}`.trim()" />
      <span id="total">{{ millis(answer.timings.total) }}</span>
      <span id="size" class="text-muted">{{ bytes(answer.size) }}</span>
      <UBadge v-if="answer.truncated" id="truncated" color="warning" variant="subtle" size="sm" :label="t('panel.response.truncated')" />
      <UBadge v-if="answer.environment" id="answer-environment" color="neutral" variant="outline" size="sm" :label="answer.environment" />
    </div>
    <VerdictList v-if="answer.checks && answer.checks.length > 0" id="verdicts" :verdicts="answer.checks" />
    <p v-if="answer.redirects.length > 0" id="redirects" class="text-xs text-muted">{{ t('panel.response.redirects', { count: answer.redirects.length, last: answer.redirects.at(-1) ?? '' }) }}</p>
    <PanelTabs id="response-tabs" v-model="tab" :items="tabs" :unmount-on-hide="false" variant="link" size="sm">
      <template #body>
        <div class="flex flex-col gap-2 pt-2">
          <USwitch v-if="json !== undefined" id="as-tree" v-model="asTree" size="sm" :label="t('panel.response.asTree')" />
          <JsonTree v-if="json !== undefined && asTree" :value="json" />
          <img v-else-if="image" id="image" :src="image" :alt="t('panel.response.image')" class="max-h-96 max-w-full self-start rounded border border-default" />
          <p v-else-if="view === 'binary'" id="binary" class="text-sm text-muted">{{ t('panel.response.binary', { size: bytes(answer.size) }) }}</p>
          <p v-else-if="answer.body === ''" id="no-body" class="text-sm text-muted">{{ t('panel.response.empty') }}</p>
          <CodeBlock v-else :text="source" :lang="lang" />
        </div>
      </template>
      <template #headers>
        <dl id="response-headers" class="grid grid-cols-[minmax(8rem,auto)_1fr] gap-x-3 gap-y-0.5 pt-2 font-mono text-xs">
          <template v-for="([name, value], index) in answer.headers" :key="index">
            <dt class="text-muted">{{ name }}</dt>
            <dd class="break-all text-highlighted">{{ value }}</dd>
          </template>
        </dl>
      </template>
      <template #cookies>
        <p v-if="cookies.length === 0" id="no-cookies" class="pt-2 text-sm text-muted">{{ t('panel.response.noCookies') }}</p>
        <ul v-else id="cookies" class="flex flex-col gap-1 pt-2 font-mono text-xs">
          <li v-for="(cookie, index) in cookies" :key="index" data-cookie class="flex flex-wrap items-center gap-1.5">
            <span class="text-highlighted">{{ cookie.name }}</span>
            <span class="break-all text-muted">{{ cookie.value }}</span>
            <UBadge v-for="attribute in cookie.attributes" :key="attribute.name" color="neutral" variant="outline" size="sm" :label="attribute.value ? `${attribute.name}=${attribute.value}` : attribute.name" />
          </li>
        </ul>
      </template>
      <template #timings>
        <dl id="timings" class="grid grid-cols-[minmax(8rem,auto)_1fr] gap-x-3 gap-y-0.5 pt-2 text-sm tabular-nums">
          <template v-for="[name, ms] in phases" :key="name">
            <dt class="text-muted">{{ t(`panel.response.phases.${name}`) }}</dt>
            <dd :data-phase="name">{{ millis(ms) }}</dd>
          </template>
        </dl>
      </template>
    </PanelTabs>
  </section>
</template>
