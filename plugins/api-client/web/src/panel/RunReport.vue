<script setup lang="ts">
// A collection run's report (RB-096-04): the counts, then every request that did not pass -
// why it got no answer, or the checks that failed. A click opens the request. The same failures
// are in the findings panel.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import VerdictList from './VerdictList.vue';
import { type Ran, type Report, failedRuns } from './checks';

const props = defineProps<{ report: Report }>();
const emit = defineEmits<{ open: [id: string]; close: [] }>();
const panel = usePanel();
const t = panel.t;

const failed = computed(() => failedRuns(props.report));

function why(ran: Ran): string {
  const error = ran.error;
  if (!error) return '';
  return panel.error(Object.assign(new Error(error.code ?? error.kind), { params: error.params ?? {} }));
}
</script>

<template>
  <section id="run-report" class="flex flex-col gap-1.5" :aria-label="t('panel.run.title')">
    <div class="flex items-center gap-1.5">
      <p class="flex-1 text-xs text-muted">{{ t('panel.run.title') }}</p>
      <UBadge v-if="report.environment" color="neutral" variant="outline" size="sm" :label="report.environment" />
      <IconAction icon="i-lucide-x" :label="t('panel.run.close')" @click="emit('close')" />
    </div>
    <p id="run-tally" class="text-sm">
      {{ t('panel.run.tally', { requests: report.tally.requests, passed: report.tally.passed, failed: report.tally.failed, errors: report.tally.errors }) }}
    </p>
    <p v-if="report.cut" id="run-cut" class="text-xs text-warning">{{ t('panel.run.cut', { count: report.requests.length }) }}</p>
    <p v-if="failed.length === 0" id="run-clean" class="text-xs text-success">{{ t('panel.run.clean') }}</p>
    <template v-else>
      <p class="text-xs text-muted">{{ t('panel.run.findings') }}</p>
      <div v-for="ran in failed" :key="ran.id" data-failed :data-run-id="ran.id" class="flex flex-col gap-0.5">
        <UButton size="xs" color="neutral" variant="ghost" class="justify-start font-normal" @click="emit('open', ran.id)">
          <span class="w-10 shrink-0 font-mono text-muted">{{ ran.status ?? t('panel.failed') }}</span>
          <span class="min-w-0 truncate">{{ ran.name || ran.id }}</span>
        </UButton>
        <p v-if="ran.error" class="ps-2 text-xs text-error">{{ why(ran) }}</p>
        <VerdictList v-else-if="ran.checks" :id="`verdicts-${ran.id}`" class="ps-2" :summary="false" :verdicts="ran.checks.filter((verdict) => !verdict.passed)" />
      </div>
    </template>
  </section>
</template>
