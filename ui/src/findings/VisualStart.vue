<script setup lang="ts">
// Visual regression's start point in the findings panel (RB-099-10), where its findings show:
// the views to compare - every view, the views the grid shows or chosen breakpoints, the one pick
// list (`ScopePicker.vue`), remembered as `visual` - and Compare (`visual.compare`) and Update
// baselines (`visual.baseline_update`). A scope leaves the baselines of the other breakpoints as
// they are; the core names the chosen breakpoints no view shows.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { FINDINGS_ICONS } from './icons';
import type { BreakpointSummary, JsonValue } from '../ipc/protocol';
import { scopeOf, withScope } from '../../../plugins/web-common/panel-ui/src/scope';
import ScopePicker from '../scope/ScopePicker.vue';
import { useScope } from '../scope/useScope';

const props = defineProps<{ breakpoints: BreakpointSummary[]; busy: boolean }>();
const emit = defineEmits<{ run: [id: string, args: Record<string, JsonValue> | null] }>();
const { t } = useI18n();
const scope = useScope('visual', () => props.breakpoints.map((breakpoint) => breakpoint.id));
const ready = computed(() => !props.busy && scopeOf(scope.value) !== null);

function run(id: string): void {
  const args = withScope({}, scope.value) as Record<string, JsonValue>;
  emit('run', id, Object.keys(args).length === 0 ? null : args);
}
</script>

<template>
  <section class="flex flex-wrap items-end gap-2" :aria-label="t('findings.visual.title')" data-testid="visual-start">
    <h3 class="w-full text-sm font-semibold">{{ t('findings.visual.title') }}</h3>
    <ScopePicker v-model="scope" :breakpoints="breakpoints" :disabled="busy" />
    <UButton size="sm" variant="outline" :disabled="!ready" data-testid="visual-compare" @click="run('visual.compare')">
      {{ t('findings.visual.compare') }}
    </UButton>
    <!-- A secondary action: an icon, named and explained by its tooltip. -->
    <UButton
      size="sm"
      variant="ghost"
      color="neutral"
      :icon="FINDINGS_ICONS.baselines"
      :aria-label="t('findings.visual.update')"
      :title="t('findings.visual.update')"
      :disabled="!ready"
      data-testid="visual-update"
      @click="run('visual.baseline_update')"
    />
  </section>
</template>
