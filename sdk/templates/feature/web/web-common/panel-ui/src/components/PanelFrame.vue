<script setup lang="ts">
// The frame of every Nuxt UI panel (RB-080-83, docs/design.md 6): `UApp` (the tooltips' and
// portals' provider), the header - icon, title, the panel's actions -, one status line, then the
// content, which scrolls on its own.
import { computed } from 'vue';
import { liveness, TONE_COLOR, TONE_ICON, type StatusLine } from '../status';

const props = defineProps<{ title: string; icon: string; status?: StatusLine | null }>();

const line = computed(() => (props.status && props.status.text ? props.status : null));
</script>

<template>
  <UApp>
    <div class="flex h-dvh flex-col bg-default text-default">
      <header class="flex items-center gap-2 border-b border-default px-3 py-2">
        <UIcon :name="icon" class="size-5 shrink-0 text-primary" />
        <h1 id="title" class="min-w-0 flex-1 truncate text-sm font-semibold text-highlighted">{{ title }}</h1>
        <div class="flex shrink-0 items-center gap-1">
          <slot name="actions" />
        </div>
      </header>
      <div id="status" role="status" :aria-live="line ? liveness(line.tone) : 'polite'" :data-tone="line?.tone">
        <UAlert
          v-if="line"
          :color="TONE_COLOR[line.tone]"
          variant="subtle"
          :icon="TONE_ICON[line.tone]"
          :title="line.text"
          :ui="{ root: 'rounded-none px-3 py-1.5', title: 'text-sm font-normal', icon: line.tone === 'busy' ? 'size-4 animate-spin' : 'size-4' }"
        />
      </div>
      <main class="min-h-0 flex-1 overflow-auto p-3">
        <slot />
      </main>
    </div>
  </UApp>
</template>
