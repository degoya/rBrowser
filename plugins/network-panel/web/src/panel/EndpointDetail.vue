<script setup lang="ts">
// The chosen endpoint (RB-096-06), in named sections: per view (calls, median, p95, waiting
// time), statuses, sizes, caching, compression and the issues it has on the current page; Show
// the requests jumps to the Requests tab filtered by the endpoint, Open in API client takes its
// newest request there (RB-096-08). A section without content is absent.
import { computed } from 'vue';
import { encodingMix, messageKey, statusMix } from './endpointModel';
import { duration, size } from './model';
import { useNetwork } from './store';

const net = useNetwork();
const ep = net.endpoints;
const t = net.panel.t;

const endpoint = computed(() => ep.endpoint.value);
const findings = computed(() => (endpoint.value ? ep.findingsOf(endpoint.value.key) : []));
const encodings = computed(() => encodingMix(endpoint.value?.encodings ?? {}));
const SEVERITY = { info: 'info', warning: 'warning', error: 'error' } as const;
</script>

<template>
  <section v-if="endpoint" id="endpoint-detail" aria-labelledby="endpoint-detail-title" class="flex min-w-0 flex-col gap-3 rounded-md border border-default p-3 md:min-h-0 md:overflow-y-auto">
    <div class="flex items-start gap-2">
      <div class="min-w-0 flex-1">
        <h2 id="endpoint-detail-title" class="font-mono text-sm break-all text-highlighted">{{ endpoint.method }} {{ endpoint.path }}</h2>
        <p class="text-xs break-all text-muted">{{ t('endpoints.origin') }}: {{ endpoint.origin }}</p>
      </div>
      <UTooltip :text="t('endpoints.close')">
        <UButton id="endpoint-close" icon="i-lucide-x" size="xs" color="neutral" variant="ghost" :aria-label="t('endpoints.close')" @click="ep.selected.value = null" />
      </UTooltip>
    </div>

    <div class="flex flex-wrap gap-1.5">
      <UButton id="endpoint-requests" size="sm" icon="i-lucide-list" :label="t('endpoints.showRequests')" @click="ep.showRequests(endpoint.key)" />
      <UButton
        v-if="endpoint.apiClient?.id"
        id="endpoint-api-open"
        size="sm"
        variant="outline"
        color="neutral"
        icon="i-lucide-send"
        :label="t('api.open')"
        @click="net.api.openRequest({ view: endpoint.apiClient.view, id: endpoint.apiClient.id })"
      />
    </div>

    <section aria-labelledby="endpoint-views-title">
      <h3 id="endpoint-views-title" class="mb-1 text-xs font-semibold text-muted uppercase">{{ t('endpoints.views') }}</h3>
      <UTable
        id="endpoint-views"
        :data="endpoint.views"
        :columns="[
          { id: 'view', header: t('endpoints.view') },
          { id: 'calls', header: t('endpoints.columns.calls'), meta: { class: { th: 'text-right', td: 'text-right tabular-nums' } } },
          { id: 'median', header: t('endpoints.columns.median'), meta: { class: { th: 'text-right', td: 'text-right tabular-nums' } } },
          { id: 'p95', header: t('endpoints.columns.p95'), meta: { class: { th: 'text-right', td: 'text-right tabular-nums' } } },
          { id: 'ttfb', header: t('endpoints.columns.ttfb'), meta: { class: { th: 'text-right', td: 'text-right tabular-nums' } } },
        ]"
        class="rounded-md border border-default"
        :ui="{ th: 'px-2 py-1 whitespace-nowrap', td: 'px-2 py-1 whitespace-nowrap text-sm' }"
      >
        <template #view-cell="{ row }">{{ ep.label(row.original.view) }}</template>
        <template #calls-cell="{ row }">{{ row.original.calls }}</template>
        <template #median-cell="{ row }">{{ duration(row.original.durationMs?.median) }}</template>
        <template #p95-cell="{ row }">{{ duration(row.original.durationMs?.p95) }}</template>
        <template #ttfb-cell="{ row }">{{ duration(row.original.ttfbMs?.p95) }}</template>
      </UTable>
    </section>

    <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-sm">
      <dt class="text-muted">{{ t('endpoints.statuses') }}</dt>
      <dd id="endpoint-statuses" class="tabular-nums">{{ statusMix(endpoint.statuses, t('endpoints.failed')) }}</dd>
      <template v-if="endpoint.transferred">
        <dt class="text-muted">{{ t('endpoints.transferred') }}</dt>
        <dd class="tabular-nums">{{ t('endpoints.sizeOf', { total: size(endpoint.transferred.total), median: size(endpoint.transferred.median) }) }}</dd>
      </template>
      <template v-if="endpoint.decoded">
        <dt class="text-muted">{{ t('endpoints.decoded') }}</dt>
        <dd class="tabular-nums">{{ t('endpoints.sizeOf', { total: size(endpoint.decoded.total), median: size(endpoint.decoded.median) }) }}</dd>
      </template>
      <dt class="text-muted">{{ t('endpoints.caching') }}</dt>
      <dd id="endpoint-caching" class="flex flex-col">
        <span v-for="value in endpoint.cacheControl" :key="value" class="font-mono text-xs break-all">{{ value }}</span>
        <span v-if="!endpoint.cacheControl.length">{{ t('endpoints.cacheNone') }}</span>
        <span v-if="endpoint.validators" class="text-muted">{{ t('endpoints.validators', { count: endpoint.validators }) }}</span>
        <span v-if="endpoint.fromCache" class="text-muted">{{ t('endpoints.fromCache', { count: endpoint.fromCache }) }}</span>
      </dd>
      <template v-if="encodings.length">
        <dt class="text-muted">{{ t('endpoints.compression') }}</dt>
        <dd id="endpoint-compression" class="flex flex-wrap gap-1">
          <UBadge v-for="one in encodings" :key="one.name" color="neutral" variant="outline" size="sm" :label="(one.name === 'none' ? t('endpoints.encodingNone') : one.name) + ' ×' + one.count" />
        </dd>
      </template>
    </dl>

    <section aria-labelledby="endpoint-findings-title">
      <h3 id="endpoint-findings-title" class="mb-1 text-xs font-semibold text-muted uppercase">{{ t('endpoints.findings') }}</h3>
      <ul v-if="findings.length" id="endpoint-findings" class="flex flex-col gap-1.5">
        <li v-for="(finding, at) in findings" :key="at" data-finding class="flex items-start gap-2 text-sm">
          <UBadge :color="SEVERITY[finding.severity] ?? 'info'" variant="subtle" size="sm" class="shrink-0" :label="t('endpoints.severity.' + finding.severity) + ' · ' + ep.label(finding.view ?? -1)" />
          <span>{{ t(messageKey(finding.message), finding.params) }}</span>
        </li>
      </ul>
      <p v-else class="text-sm text-muted">{{ t('endpoints.none') }}</p>
    </section>
  </section>
</template>
