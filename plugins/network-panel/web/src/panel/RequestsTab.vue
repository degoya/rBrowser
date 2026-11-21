<script setup lang="ts">
// The Requests tab (RB-070-02, on Nuxt UI since RB-080-84): the filters (address, type, status,
// differences only), the session's switches for mocks and the cache - each saying its state in
// words and as pressed -, what the list cannot show (notes, each once), the requests and the
// selected one's detail beside them (below them when narrow). A saved session open instead of
// the live list says so in the status line, with Back to live requests here. The Endpoints
// tab's jump filters the list by an endpoint (RB-096-06), shown above it with the way back. Save
// shown as requests takes the live list as filtered into the API client (RB-096-08).
import { computed, watch } from 'vue';
import { STATUSES, TYPES } from './model';
import RequestDetail from './RequestDetail.vue';
import RequestTable from './RequestTable.vue';
import { useNetwork } from './store';

const net = useNetwork();
const t = net.panel.t;

const ALL = 'all';
const typeItems = computed(() => [{ label: t('toolbar.allTypes'), value: ALL }, ...TYPES.map((type) => ({ label: t('types.' + type.toLowerCase()), value: type }))]);
const statusItems = computed(() => STATUSES.map((status) => ({ label: t('statuses.' + status), value: status })));
const type = computed({
  get: () => net.filters.type || ALL,
  set: (value: string) => {
    net.filters.type = value === ALL ? '' : value;
  },
});

watch(
  () => ({ ...net.filters }),
  () => void net.refresh(),
);

const mocksOn = computed(() => net.answer.value?.mocksEnabled ?? true);
const cacheOff = computed(() => net.answer.value?.cacheDisabled ?? false);

/** What the list cannot show or apply, each named once. */
const notes = computed(() => {
  const answer = net.answer.value;
  if (!answer || net.session.value) return [];
  const out: { key: string; tone: 'warning' | 'info'; text: string }[] = [];
  if (!answer.captured) out.push({ key: 'capture', tone: 'warning', text: t('status.noCapture') });
  if (!answer.rulesRunning) out.push({ key: 'rules', tone: 'warning', text: t('status.noRules') });
  if (!answer.trusted) out.push({ key: 'trust', tone: 'warning', text: t('status.untrusted') });
  const unsupported = answer.views.filter((view) => view.supported === false).map((view) => view.label);
  if (unsupported.length) out.push({ key: 'unsupported', tone: 'info', text: t('status.unsupported', { views: unsupported.join(', ') }) });
  if (answer.total > answer.rows.length) out.push({ key: 'truncated', tone: 'info', text: t('status.truncated', { shown: answer.rows.length, total: answer.total }) });
  return out;
});
</script>

<template>
  <div id="panel-requests" class="flex flex-col gap-2 md:min-h-0 md:flex-1">
    <div class="flex flex-wrap items-center gap-2">
      <UInput
        id="search"
        v-model="net.filters.search"
        type="search"
        icon="i-lucide-search"
        size="sm"
        :maxlength="200"
        autocomplete="off"
        :spellcheck="false"
        :placeholder="t('toolbar.search')"
        :aria-label="t('toolbar.search')"
        class="min-w-40 flex-1"
      />
      <USelect id="type" v-model="type" :items="typeItems" size="sm" :aria-label="t('toolbar.type')" class="w-36" />
      <USelect id="status-filter" v-model="net.filters.status" :items="statusItems" size="sm" :aria-label="t('toolbar.status')" class="w-36" />
      <UCheckbox id="differences" v-model="net.filters.differences" size="sm" :label="t('toolbar.differences')" />
    </div>
    <div class="flex flex-wrap items-center gap-1.5">
      <UButton
        id="mocks-toggle"
        size="xs"
        icon="i-lucide-flask-conical"
        :color="mocksOn ? 'primary' : 'neutral'"
        :variant="mocksOn ? 'soft' : 'outline'"
        :aria-pressed="mocksOn"
        :label="t(mocksOn ? 'toolbar.mocksOn' : 'toolbar.mocksOff')"
        @click="net.act({ command: 'mocks_toggle' }, {}, 'done.mocks')"
      />
      <UButton
        id="cache-toggle"
        size="xs"
        icon="i-lucide-database"
        :color="cacheOff ? 'primary' : 'neutral'"
        :variant="cacheOff ? 'soft' : 'outline'"
        :aria-pressed="cacheOff"
        :label="t(cacheOff ? 'toolbar.cacheOff' : 'toolbar.cacheOn')"
        @click="net.act({ command: 'cache_toggle' }, {}, 'done.cache')"
      />
      <UButton
        v-if="!net.session.value"
        id="api-save-shown"
        size="xs"
        color="neutral"
        variant="outline"
        icon="i-lucide-save-all"
        :disabled="!(net.answer.value?.rows.length ?? 0)"
        :label="t('api.saveShown')"
        @click="net.api.saveShown()"
      />
      <UButton
        v-if="net.session.value"
        id="session-close"
        size="xs"
        color="primary"
        variant="soft"
        icon="i-lucide-arrow-left"
        :label="t('sessions.close')"
        @click="net.sessions.close()"
      />
    </div>

    <div v-if="net.filters.endpoint" id="endpoint-filter" class="flex min-w-0 flex-wrap items-center gap-1.5">
      <UBadge color="primary" variant="subtle" icon="i-lucide-route" class="min-w-0" :ui="{ label: 'truncate' }" :label="t('endpoints.filtered', { endpoint: net.filters.endpoint })" />
      <UButton id="endpoint-filter-clear" size="xs" variant="ghost" color="neutral" icon="i-lucide-x" :label="t('endpoints.clearFilter')" @click="net.filters.endpoint = ''" />
    </div>

    <div v-if="notes.length" id="notes" class="flex flex-col gap-1.5">
      <UAlert
        v-for="note in notes"
        :key="note.key"
        :data-note="note.key"
        :color="note.tone"
        variant="subtle"
        :icon="note.tone === 'warning' ? 'i-lucide-triangle-alert' : 'i-lucide-info'"
        :title="note.text"
        :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }"
      />
    </div>

    <div class="grid grid-cols-[minmax(0,1fr)] gap-3 md:min-h-0 md:flex-1 md:grid-rows-[minmax(0,1fr)]" :class="net.selectedRow.value ? 'md:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]' : undefined">
      <RequestTable />
      <RequestDetail v-if="net.selectedRow.value" />
    </div>
  </div>
</template>
