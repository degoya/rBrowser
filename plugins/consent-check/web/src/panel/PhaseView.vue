<script setup lang="ts">
// One phase of a consent check (RB-094-05): the requests to other sites by host and kind (an
// allowed host marked), the count of the page's own requests, the cookies with their lifetime
// (never a value; essential and other-site ones marked) and the localStorage keys.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Phase, hostLabel, lifetime, text } from './model';

defineProps<{ phase: Phase }>();
const { strings } = usePanel();
</script>

<template>
  <div class="flex flex-col gap-4">
    <section class="flex flex-col gap-1.5">
      <h2 class="text-sm font-semibold text-highlighted">{{ text(strings, 'panel.hosts') }}</h2>
      <p v-if="phase.hosts.length === 0" class="text-sm text-muted">{{ text(strings, 'panel.noHosts') }}</p>
      <ul v-else class="flex flex-col gap-1">
        <li v-for="row in phase.hosts" :key="`${row.host} ${row.kind}`" data-host class="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-sm">
          <UIcon name="i-lucide-globe" class="size-4 shrink-0 text-muted" />
          <span class="font-medium break-all">{{ row.host }}</span>
          <span class="text-muted">{{ hostLabel(strings, row) }}</span>
          <UBadge v-if="row.allowed" color="neutral" variant="subtle" size="sm" :label="text(strings, 'panel.allowed')" />
          <span class="w-full truncate pl-6 text-xs text-muted" :title="row.example">{{ row.example }}</span>
        </li>
      </ul>
      <p class="text-xs text-muted">{{ text(strings, 'panel.firstParty', { count: phase.firstParty }) }}</p>
    </section>

    <section class="flex flex-col gap-1.5">
      <h2 class="text-sm font-semibold text-highlighted">{{ text(strings, 'panel.cookies') }}</h2>
      <p v-if="phase.cookies.length === 0" class="text-sm text-muted">{{ text(strings, 'panel.noCookies') }}</p>
      <ul v-else class="flex flex-col gap-1">
        <li v-for="cookie in phase.cookies" :key="`${cookie.name} ${cookie.domain}`" data-cookie class="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-sm">
          <UIcon name="i-lucide-cookie" class="size-4 shrink-0 text-muted" />
          <span class="font-medium break-all">{{ cookie.name }}</span>
          <span class="text-muted break-all">{{ cookie.domain }}</span>
          <span class="text-muted">{{ lifetime(strings, cookie.days) }}</span>
          <UBadge v-if="cookie.essential" color="success" variant="subtle" size="sm" :label="text(strings, 'panel.essential')" />
          <UBadge v-if="cookie.thirdParty" color="warning" variant="subtle" size="sm" :label="text(strings, 'panel.thirdParty')" />
        </li>
      </ul>
    </section>

    <section class="flex flex-col gap-1.5">
      <h2 class="text-sm font-semibold text-highlighted">{{ text(strings, 'panel.storage') }}</h2>
      <p v-if="phase.storage.length === 0" class="text-sm text-muted">{{ text(strings, 'panel.noStorage') }}</p>
      <ul v-else class="flex flex-wrap gap-1.5">
        <li v-for="key in phase.storage" :key="key" data-storage>
          <UBadge color="neutral" variant="outline" size="sm" :label="key" />
        </li>
      </ul>
    </section>
  </div>
</template>
