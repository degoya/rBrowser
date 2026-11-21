<script setup lang="ts">
// The selected request (RB-070-02, on Nuxt UI since RB-080-84): its address, a line per view
// (status with MIME type and cache below it, size, time - or that the view did not load it) with Show for its
// response body, the body in the syntax colours of its MIME type (the one highlighter, text
// only), and the request's actions: Mock this request, Repeat, Repeat 10x, Open in API client and
// Save as request (RB-096-08; live list only).
import { computed, ref, watch } from 'vue';
import type { TableColumn } from '@nuxt/ui';
import CodeBlock from '../../../../web-common/panel-ui/src/code/CodeBlock.vue';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import { duration, keyOf, shortUrl, size } from './model';
import type { ListView, Seen } from './types';
import { useNetwork } from './store';

const net = useNetwork();
const t = net.panel.t;
const row = computed(() => net.selectedRow.value);

type Line = { view: ListView; seen: Seen | undefined };
const lines = computed<Line[]>(() => (net.shown.value?.views ?? []).map((view) => ({ view, seen: row.value?.views[String(view.id)] })));

const columns = computed<TableColumn<Line>[]>(() =>
  ['view', 'status', 'size', 'time', 'body'].map((id) => ({ id, header: t('detail.' + id) })),
);

/** The body shown: its text or why there is none. */
const body = ref<{ text: string; base64: boolean; truncated: boolean; mime: string } | null>(null);
const bodyError = ref('');

watch(
  () => (row.value ? keyOf(row.value) : null),
  () => {
    body.value = null;
    bodyError.value = '';
  },
);

function show(view: number, seen: Seen): void {
  const mime = seen.mimeType ?? '';
  net.panel.call({ tool: 'body' }, { view, id: seen.id }).then(
    (value) => {
      bodyError.value = '';
      body.value = { ...(value as { text: string; base64: boolean; truncated: boolean }), mime };
    },
    (error: unknown) => {
      body.value = null;
      bodyError.value = net.panel.error(error);
    },
  );
}

const lang = computed(() => (body.value && !body.value.base64 ? highlighter().detect(body.value.text, body.value.mime) : null));
</script>

<template>
  <UCard
    v-if="row"
    id="detail"
    as="section"
    aria-labelledby="detail-title"
    class="md:min-h-0 md:overflow-auto"
    :ui="{ header: 'flex items-start gap-2 px-3 py-2 sm:px-3', body: 'flex flex-col gap-3 p-3 sm:p-3' }"
  >
    <template #header>
      <div class="min-w-0 flex-1">
        <h2 id="detail-title" class="truncate text-sm font-semibold text-highlighted">{{ row.method + ' ' + shortUrl(row.url) }}</h2>
        <p id="detail-url" class="font-mono text-xs break-all text-muted">{{ row.url }}</p>
      </div>
      <IconAction id="detail-close" icon="i-lucide-x" :label="t('detail.close')" @click="net.selected.value = null" />
    </template>

    <UTable id="detail-views" :data="lines" :columns="columns" :ui="{ th: 'px-2 py-1 whitespace-nowrap', td: 'px-2 py-1 whitespace-nowrap text-xs' }">
      <template #view-cell="{ row: line }">{{ line.original.view.label }}</template>
      <template #status-cell="{ row: line }">
        <span v-if="!line.original.seen" data-missing class="text-muted">{{ t('detail.notLoaded') }}</span>
        <div v-else class="flex flex-col">
          <span>{{ String(line.original.seen.status ?? line.original.seen.failed ?? '-') }}</span>
          <span data-type class="text-muted">{{ [line.original.seen.mimeType ?? '-', line.original.seen.fromCache ? t('detail.fromCache') : ''].filter(Boolean).join(' · ') }}</span>
        </div>
      </template>
      <template #size-cell="{ row: line }">{{ line.original.seen ? size(line.original.seen.size) : '' }}</template>
      <template #time-cell="{ row: line }">{{ line.original.seen ? duration(line.original.seen.durationMs) : '' }}</template>
      <template #body-cell="{ row: line }">
        <UButton
          v-if="line.original.seen"
          size="xs"
          variant="link"
          color="primary"
          data-show-body
          :data-view="line.original.view.id"
          :label="t('detail.showBody')"
          @click="show(line.original.view.id, line.original.seen)"
        />
      </template>
    </UTable>

    <UAlert v-if="bodyError" id="body-error" color="error" variant="subtle" icon="i-lucide-circle-x" :title="bodyError" :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }" />
    <div v-if="body" id="body" class="flex flex-col gap-1">
      <p v-if="body.base64" data-binary class="text-sm text-muted">{{ t('detail.binary') }}</p>
      <CodeBlock v-else :text="body.text" :lang="lang" :note="t('detail.plainLarge')" />
      <p v-if="!body.base64 && body.truncated" id="body-cut" class="text-xs text-muted italic">{{ t('detail.cut') }}</p>
    </div>

    <div v-if="!net.session.value" class="flex flex-wrap gap-1.5">
      <UButton id="mock-this" size="sm" variant="outline" color="neutral" icon="i-lucide-flask-conical" :label="t('detail.mockThis')" @click="net.mockThis()" />
      <UButton id="repeat-this" size="sm" variant="outline" color="neutral" icon="i-lucide-repeat" :label="t('compose.repeat')" @click="net.repeat(1)" />
      <UButton id="repeat-ten" size="sm" variant="outline" color="neutral" icon="i-lucide-repeat-2" :label="t('compose.repeatTen')" @click="net.repeat(10)" />
      <UButton id="api-open" size="sm" variant="outline" color="neutral" icon="i-lucide-send" :label="t('api.open')" @click="net.api.open()" />
      <UButton id="api-save" size="sm" variant="outline" color="neutral" icon="i-lucide-save" :label="t('api.save')" @click="net.api.save()" />
    </div>
  </UCard>
</template>
