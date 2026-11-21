<script setup lang="ts">
// The verdicts of a request's checks (RB-096-04): how many passed, then one line per check - a
// pass or fail mark, the check as its file has it, and what was found (masked by the core) or
// why nothing was.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { Verdict } from './checks';

const props = withDefaults(defineProps<{ id: string; verdicts: Verdict[]; summary?: boolean }>(), { summary: true });
const t = usePanel().t;

const passed = computed(() => props.verdicts.filter((verdict) => verdict.passed).length);

function found(verdict: Verdict): string {
  if (verdict.problem) return t(`panel.verdicts.problems.${verdict.problem}`);
  return verdict.actual === undefined ? '' : t('panel.verdicts.actual', { actual: verdict.actual });
}
</script>

<template>
  <section :id="id" class="flex flex-col gap-1" :aria-label="t('panel.verdicts.title')">
    <p v-if="summary" class="text-xs text-muted" data-verdict-summary>{{ t('panel.verdicts.summary', { count: verdicts.length, passed }) }}</p>
    <ul class="flex flex-col gap-0.5 text-xs">
      <li v-for="(verdict, index) in verdicts" :key="index" data-verdict :data-passed="verdict.passed" class="flex min-w-0 items-start gap-1.5">
        <UIcon
          :name="verdict.passed ? 'i-lucide-circle-check' : 'i-lucide-circle-x'"
          :class="verdict.passed ? 'text-success' : 'text-error'"
          class="mt-0.5 size-3.5 shrink-0"
          :aria-label="verdict.passed ? t('panel.verdicts.pass') : t('panel.verdicts.fail')"
        />
        <span class="shrink-0 font-mono text-highlighted">{{ verdict.check }}</span>
        <span v-if="!verdict.passed" class="min-w-0 break-all text-muted">{{ found(verdict) }}</span>
      </li>
    </ul>
  </section>
</template>
