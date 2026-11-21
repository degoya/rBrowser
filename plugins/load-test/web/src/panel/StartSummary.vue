<script setup lang="ts">
// What a run will do, before it does (RB-096-07, a review condition): its targets with their
// standing, users, rate and duration. A target that is neither the project's own nor this
// machine is named with a box the person ticks - the host is theirs, or they may test it - and
// only their Confirm keeps it; Start stays off until every target may be one. Start sends back
// exactly what is shown here, and the core starts nothing else.
import { computed, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Summary, clock } from './model';

const props = defineProps<{ summary: Summary; busy: boolean }>();
const emit = defineEmits<{ confirm: [host: string]; start: []; close: [] }>();
const t = usePanel().t;

const ticked = ref<Record<string, boolean>>({});
const blocked = computed(() => props.summary.unconfirmed.length > 0);
const ramp = computed(() => props.summary.rampUpS > 0 || props.summary.rampDownS > 0);
</script>

<template>
  <section id="start-summary" class="flex flex-col gap-2 rounded border border-default p-3" :aria-label="t('panel.summary.title')">
    <p class="text-sm font-medium">{{ t('panel.summary.title') }}</p>
    <dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
      <dt class="text-muted">{{ t('panel.summary.targets') }}</dt>
      <dd id="summary-targets" class="flex flex-wrap gap-1">
        <UBadge v-for="target in summary.targets" :key="target.host" :data-host="target.host" :data-standing="target.standing" size="sm" variant="subtle" :color="target.standing === 'unconfirmed' ? 'warning' : 'neutral'">
          {{ t(`panel.summary.standing.${target.standing}`, { host: target.host }) }}
        </UBadge>
      </dd>
      <dt class="text-muted">{{ t('panel.summary.users') }}</dt>
      <dd id="summary-users">{{ summary.users }}</dd>
      <dt class="text-muted">{{ t('panel.summary.rate') }}</dt>
      <dd id="summary-rate">{{ t('panel.summary.rateValue', { rps: summary.rps }) }}</dd>
      <dt class="text-muted">{{ t('panel.summary.duration') }}</dt>
      <dd id="summary-duration">
        {{ ramp ? t('panel.summary.ramp', { total: clock(summary.durationS), up: summary.rampUpS, hold: summary.holdS, down: summary.rampDownS }) : clock(summary.durationS) }}
      </dd>
      <template v-if="summary.environment">
        <dt class="text-muted">{{ t('panel.summary.environment') }}</dt>
        <dd>{{ summary.environment }}</dd>
      </template>
    </dl>

    <div v-for="host in summary.unconfirmed" :key="host" class="flex flex-wrap items-center gap-2 rounded bg-elevated p-2" :data-unconfirmed="host" role="group" :aria-label="t('panel.summary.confirmTitle', { host })">
      <UCheckbox v-model="ticked[host]" :label="t('panel.summary.confirmBox', { host })" />
      <UButton size="xs" color="neutral" variant="outline" :disabled="!ticked[host] || busy" :label="t('panel.summary.confirmHost')" data-confirm @click="emit('confirm', host)" />
    </div>

    <p class="text-xs text-muted">{{ t('panel.summary.note') }}</p>
    <div class="flex flex-wrap items-center gap-2">
      <UButton id="start" size="sm" color="error" icon="i-lucide-play" :disabled="blocked || busy" :label="t('panel.summary.start')" @click="emit('start')" />
      <UButton id="summary-close" size="sm" color="neutral" variant="ghost" :label="t('panel.summary.notNow')" @click="emit('close')" />
    </div>
  </section>
</template>
