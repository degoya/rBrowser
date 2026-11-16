<script setup lang="ts">
// The which-key overlay: after the leader it covers the toolbar strip (the shell view, never the
// page) with what was typed and each next key - a group or a command. It takes no focus and no
// clicks; the keys go to the core as before. The coordinator mounts it in `App.vue`.
import { useI18n } from 'vue-i18n';
import UKbd from '@nuxt/ui/components/Kbd.vue';
import { useWhichKey, windowEvents, type Subscribe } from './whichkey';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const { t } = useI18n();
const which = useWhichKey(props.subscribe ?? windowEvents);
</script>

<template>
  <!-- The live region stays in the DOM, so the bar's first keys are announced (RB-090-22). -->
  <div role="status" aria-live="polite" :aria-label="t('shell.whichkey.label')" data-testid="which-key-region">
    <div
      v-if="which !== null"
      class="pointer-events-none fixed inset-x-0 top-0 z-50 flex h-11 items-center gap-3 overflow-hidden border-b border-default bg-elevated px-3 text-sm text-default"
      data-testid="which-key"
    >
      <span class="shrink-0 font-mono text-xs text-muted" data-testid="which-key-typed">{{ which.typed }}</span>
      <ul class="flex min-w-0 items-center gap-3 overflow-hidden whitespace-nowrap">
        <li
          v-for="entry in which.entries"
          :key="entry.key"
          class="flex items-center gap-1"
          :data-kind="entry.kind"
        >
          <UKbd :value="entry.key" size="sm" />
          <span :class="entry.kind === 'group' ? 'font-semibold text-highlighted' : 'text-default'">
            {{ t(entry.titleKey) }}
          </span>
        </li>
      </ul>
      <span class="ml-auto shrink-0 text-xs text-muted">{{ t('shell.whichkey.cancel') }}</span>
    </div>
  </div>
</template>
