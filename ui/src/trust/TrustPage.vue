<script setup lang="ts">
// The trust dialog (route `trust` of the pages window, RB-010-09) and the list of trusted
// projects for Settings > Security & Privacy > Workspace trust: what the open project would
// run or redirect, the one button that trusts it, and revoking.
import { computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { PluginGrantDetails } from '../ipc/protocol';
import { kindKey, useTrust } from './trust';
import RiskyNote from './RiskyNote.vue';
import TrustGrants from './TrustGrants.vue';

const trust = useTrust();
const { t } = useI18n();

const current = computed(() => trust.info.value?.current ?? null);
const status = computed(() => {
  const project = current.value;
  if (project === null) return 'trust.page.no_project';
  if (project.trusted) return 'trust.page.status_trusted';
  return project.blocked.length === 0 ? 'trust.page.status_nothing' : 'trust.page.status_restricted';
});
const canDecide = computed(
  () =>
    trust.prompt.value !== null &&
    current.value !== null &&
    !current.value.trusted &&
    trust.prompt.value.projectRoot === current.value.projectRoot,
);
// What a project plugin's manifest says it may do, shown before the person trusts it.
const capabilitiesOf = (path: string): string[] =>
  current.value?.pluginCapabilities?.[path] ?? [];
// A capability the core names risky (`network-rules`, `console-logs`; review L3 of Charge 7;
// `sidecar`, security review RB-090-06).
const riskyCapability = (path: string, capability: string): boolean =>
  current.value?.pluginGrants?.[path]?.risky.includes(capability) ?? false;
// Why it is risky: the sidecar runs Lighthouse and Playwright in Node, the others see the traffic.
const riskyTitle = (capability: string): string =>
  capability === 'sidecar'
    ? t('trust.page.grants.risky_sidecar')
    : capability === 'project-write'
      ? t('trust.page.grants.risky_write')
      : t('trust.page.grants.risky_capability');
// The same in detail, the risky parts highlighted (review M1 of Charge 6).
const grantsOf = (path: string): PluginGrantDetails | null => {
  const grants = current.value?.pluginGrants?.[path];
  if (grants === undefined) return null;
  const any =
    grants.evaluate ||
    grants.cdpDomains.length > 0 ||
    grants.projectGlobs.length > 0 ||
    (grants.writeGlobs ?? []).length > 0 ||
    grants.sidecarMethods.length > 0 ||
    grants.httpRatePerSecond !== undefined;
  return any ? grants : null;
};

const network = new Set(['hostResolverRules', 'ignoreCertificateErrors', 'proxy']);
const hasNetwork = computed(
  () => current.value?.blocked.some((item) => network.has(item.kind)) ?? false,
);

// The store may be the window's own (started in main.ts); starting it again changes nothing.
onMounted(() => {
  trust.start();
  void trust.refresh();
});
</script>

<template>
  <section class="flex flex-col gap-4 p-4 text-default" aria-labelledby="trust-title">
    <h1 id="trust-title" class="text-lg font-semibold">{{ t('trust.page.title') }}</h1>

    <div v-if="current !== null" class="flex flex-col gap-1">
      <span class="text-sm text-muted">{{ t('trust.page.project') }}</span>
      <code class="break-all" data-testid="trust-root">{{ current.projectRoot }}</code>
    </div>
    <p data-testid="trust-status">{{ t(status) }}</p>

    <div v-if="current !== null && current.blocked.length > 0" class="flex flex-col gap-2">
      <h2 class="font-medium">{{ t('trust.page.blocked') }}</h2>
      <ul class="flex flex-col gap-1" data-testid="trust-blocked">
        <li
          v-for="item in current.blocked"
          :key="`${item.kind}:${item.path}`"
          class="flex flex-wrap gap-2"
        >
          <span class="shrink-0">{{ t(kindKey(item)) }}</span>
          <code class="break-all text-muted">{{ item.path }}</code>
          <ul
            v-if="capabilitiesOf(item.path).length > 0"
            class="w-full list-disc ps-6 text-sm text-muted"
            data-testid="trust-capabilities"
          >
            <li
              v-for="capability in capabilitiesOf(item.path)"
              :key="capability"
              :class="riskyCapability(item.path, capability) ? 'font-semibold text-warning' : undefined"
              :data-risky="riskyCapability(item.path, capability) ? 'true' : undefined"
            >
              <span data-testid="trust-capability">{{ t(`plugins.capability.${capability}`) }}</span>
              <RiskyNote
                v-if="riskyCapability(item.path, capability)"
                :reason="riskyTitle(capability)"
              />
            </li>
          </ul>
          <TrustGrants
            v-if="grantsOf(item.path) !== null"
            :grants="grantsOf(item.path)!"
          />
        </li>
      </ul>
      <p class="text-sm text-muted">{{ t('trust.page.change_note') }}</p>
      <p v-if="hasNetwork" class="text-sm text-muted">{{ t('trust.page.network_note') }}</p>
    </div>

    <p v-if="canDecide && current?.blocked.length === 0" class="text-sm text-muted">
      {{ t('trust.page.origins_note') }}
    </p>
    <div v-if="canDecide" class="flex gap-2">
      <UButton color="warning" data-testid="trust-grant" @click="trust.decide('trust')">
        {{ t('trust.page.grant') }}
      </UButton>
      <UButton
        color="neutral"
        variant="outline"
        data-testid="trust-restricted"
        @click="trust.decide('restricted')"
      >
        {{ t('trust.page.restricted') }}
      </UButton>
    </div>

    <p v-if="trust.failed.value" role="alert" class="text-error">{{ t('trust.page.failed') }}</p>

    <div class="flex flex-col gap-2">
      <h2 class="font-medium">{{ t('trust.page.projects') }}</h2>
      <p v-if="(trust.info.value?.projects.length ?? 0) === 0" class="text-sm text-muted">
        {{ t('trust.page.projects_none') }}
      </p>
      <ul v-else class="flex flex-col gap-2" data-testid="trust-projects">
        <li
          v-for="project in trust.info.value?.projects ?? []"
          :key="project.root"
          class="flex items-center gap-2"
        >
          <div class="flex min-w-0 flex-1 flex-col">
            <code class="break-all">{{ project.root }}</code>
            <span class="text-sm text-muted">
              {{ t('trust.page.decided_at', { date: project.decidedAt }) }}
            </span>
          </div>
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            :aria-label="t('trust.page.revoke_label', { root: project.root })"
            data-testid="trust-revoke"
            @click="trust.revoke(project.root)"
          >
            {{ t('trust.page.revoke') }}
          </UButton>
        </li>
      </ul>
    </div>
  </section>
</template>
