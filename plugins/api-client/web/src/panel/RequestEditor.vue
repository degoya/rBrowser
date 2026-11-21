<script setup lang="ts">
// The request's tabs (RB-096-01): the parameters (the URL's query as rows, both kept in step),
// the headers, the body, the auth, the checks and the options. The method, the URL and Send are
// the fixed header's above the tabs (`RequestHeader.vue`, RB-096-11).
import { computed, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import AuthEditor from './AuthEditor.vue';
import BodyEditor from './BodyEditor.vue';
import ChecksEditor from './ChecksEditor.vue';
import KeyValueTable from './KeyValueTable.vue';
import type { ApiRequest, Entry } from './model';
import { joinUrl, splitUrl } from './query';

const request = defineModel<ApiRequest>({ required: true });
const t = usePanel().t;

const TABS = ['params', 'headers', 'body', 'auth', 'checks', 'options'] as const;
const tab = ref<string>('params');
const tabs = computed(() =>
  TABS.map((name) => ({
    label: t(`panel.tabs.${name}`),
    value: name,
    slot: name,
    badge: badgeOf(name),
  })),
);
function badgeOf(name: (typeof TABS)[number]): string | undefined {
  const count = name === 'headers' ? request.value.headers.length : name === 'checks' ? (request.value.checks ?? []).length : 0;
  return count > 0 ? String(count) : undefined;
}

function patch(next: Partial<ApiRequest>): void {
  request.value = { ...request.value, ...next };
}

const params = computed<Entry[]>({
  get: () => splitUrl(request.value.url).params.map((param) => ({ ...param, enabled: true })),
  set: (rows) => {
    const split = splitUrl(request.value.url);
    patch({ url: joinUrl(split.base, rows, split.hash) });
  },
});

function setTimeoutMs(text: string | number): void {
  const ms = Number(text);
  if (Number.isFinite(ms) && ms > 0) patch({ options: { ...request.value.options, timeoutMs: Math.min(Math.round(ms), 600000) } });
}
</script>

<template>
  <div id="request" class="flex flex-col gap-2">
    <PanelTabs id="request-tabs" v-model="tab" :items="tabs" :unmount-on-hide="false" variant="link" size="sm">
      <template #params>
        <KeyValueTable id="params" v-model="params" class="pt-2" />
      </template>
      <template #headers>
        <KeyValueTable id="headers" toggles :model-value="request.headers" class="pt-2" @update:model-value="(headers) => patch({ headers })" />
      </template>
      <template #body>
        <BodyEditor :model-value="request.body" @update:model-value="(body) => patch({ body })" />
      </template>
      <template #auth>
        <AuthEditor :model-value="request.auth" @update:model-value="(auth) => patch({ auth })" />
      </template>
      <template #checks>
        <ChecksEditor :model-value="request.checks ?? []" @update:model-value="(checks) => patch({ checks })" />
      </template>
      <template #options>
        <div id="options" class="flex flex-col gap-2 pt-2">
          <UFormField :label="t('panel.options.timeout')" name="timeout" size="sm">
            <UInput id="timeout" type="number" :model-value="request.options.timeoutMs" size="sm" class="w-36" min="1" max="600000" @update:model-value="setTimeoutMs" />
          </UFormField>
          <USwitch
            id="redirects"
            :model-value="request.options.followRedirects"
            :label="t('panel.options.redirects')"
            :description="t('panel.options.redirectsHint')"
            @update:model-value="(on) => patch({ options: { ...request.options, followRedirects: on === true } })"
          />
          <USwitch
            id="tls"
            :model-value="request.options.verifyTls"
            :label="t('panel.options.tls')"
            :description="t('panel.options.tlsHint')"
            @update:model-value="(on) => patch({ options: { ...request.options, verifyTls: on === true } })"
          />
        </div>
      </template>
    </PanelTabs>
  </div>
</template>
