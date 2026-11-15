<script setup lang="ts">
// A project plugin's grants in detail under its package in the trust dialog (review M1 of
// Charge 6): what `evaluate`, the CDP domains, the globs, the sidecar methods and the HTTP rate
// allow, the risky parts highlighted - `evaluate` is full control of the page, its network
// included; the sidecar methods run in Node (security review RB-090-06) - and both are marked with
// a word and their reason (`RiskyNote.vue`, RB-090-22).
import { useI18n } from 'vue-i18n';
import type { PluginGrantDetails } from '../ipc/protocol';
import RiskyNote from './RiskyNote.vue';

const props = defineProps<{ grants: PluginGrantDetails }>();
const { t } = useI18n();

const risky = (domain: string): boolean => props.grants.risky.includes(`cdp:${domain}`);
// The sidecar runs Lighthouse and Playwright in Node (security review RB-090-06).
const riskySidecar = (): boolean => props.grants.risky.includes('sidecar');
</script>

<template>
  <ul class="w-full list-disc ps-6 text-sm text-muted" data-testid="trust-grants">
    <li v-if="grants.evaluate" class="font-semibold text-warning" data-risky="true">
      {{ t('trust.page.grants.evaluate') }}
      <RiskyNote />
    </li>
    <li v-if="grants.cdpDomains.length > 0">
      {{ t('trust.page.grants.cdp') }}
      <template v-for="(domain, index) in grants.cdpDomains" :key="domain">
        <span v-if="index > 0">, </span>
        <template v-if="risky(domain)">
          <code class="font-semibold text-warning" data-risky="true">{{ domain }}</code>
          <RiskyNote :reason="t('trust.page.grants.risky_cdp')" />
        </template>
        <code v-else>{{ domain }}</code>
      </template>
    </li>
    <li v-if="grants.projectGlobs.length > 0">
      {{ t('trust.page.grants.globs') }}
      <code class="break-all">{{ grants.projectGlobs.join(', ') }}</code>
    </li>
    <li
      v-if="grants.sidecarMethods.length > 0"
      :class="riskySidecar() ? 'font-semibold text-warning' : undefined"
      :data-risky="riskySidecar() ? 'true' : undefined"
    >
      {{ t('trust.page.grants.sidecar') }}
      <code class="break-all">{{ grants.sidecarMethods.join(', ') }}</code>
      <RiskyNote v-if="riskySidecar()" :reason="t('trust.page.grants.risky_sidecar')" />
    </li>
    <li v-if="grants.httpRatePerSecond !== undefined">
      {{ t('trust.page.grants.http_rate', { rate: grants.httpRatePerSecond }) }}
    </li>
  </ul>
</template>
