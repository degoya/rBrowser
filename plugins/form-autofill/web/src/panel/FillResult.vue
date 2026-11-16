<script setup lang="ts">
// A fill's result (RB-080-86): what the fill was - the values, the profile, how the forms
// checked, the submits cancelled - and its seed with a copy button, then the fields: a card per
// form when the page has several, else one list.
import { computed, onBeforeUnmount, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import FieldTable from './FieldTable.vue';
import { type AutofillResult, formState, type Outcome } from './model';

const props = defineProps<{ result: AutofillResult; seen: Outcome }>();
const { t } = usePanel();

const state = computed(() => formState(t, props.seen.filled));
const blocked = computed(() => props.seen.filled.reduce((sum, view) => sum + (view.blockedSubmits ?? 0), 0));
const several = computed(() => props.seen.sections.length > 1);

const copied = ref(false);
let timer: ReturnType<typeof setTimeout> | undefined;
async function copySeed(): Promise<void> {
  try {
    await navigator.clipboard.writeText(String(props.result.seed));
    copied.value = true;
    clearTimeout(timer);
    timer = setTimeout(() => (copied.value = false), 2000);
  } catch {
    // The frame may write to the clipboard only; the seed stays on screen to select.
  }
}
onBeforeUnmount(() => clearTimeout(timer));
</script>

<template>
  <section id="results" class="flex flex-col gap-3" :aria-label="t('panel.results')">
    <div id="facts" class="flex flex-wrap items-center gap-1.5">
      <UBadge data-fact="mode" :color="result.mode === 'valid' ? 'success' : 'warning'" variant="subtle" :label="t('panel.modes.' + result.mode)" />
      <UBadge v-if="result.mode === 'invalid' && result.strategy" data-fact="strategy" color="neutral" variant="subtle" :label="t('panel.strategies.' + result.strategy)" />
      <UBadge v-if="result.profile" data-fact="profile" color="info" variant="subtle" class="max-w-full" :label="t('panel.profileUsed', { name: result.profile })" />
      <UBadge v-if="state" data-fact="form" :color="state.color" variant="subtle" :label="state.text" />
      <UBadge v-if="blocked > 0" data-fact="blocked" color="warning" variant="subtle" icon="i-lucide-ban" :label="t('panel.blocked', { count: blocked })" />
      <span class="ms-auto flex items-center gap-0.5">
        <span id="seed-used" class="select-all text-sm tabular-nums text-muted">{{ t('panel.seedUsed', { seed: result.seed }) }}</span>
        <IconAction id="copy-seed" :icon="copied ? 'i-lucide-check' : 'i-lucide-copy'" :label="copied ? t('panel.seedCopied') : t('panel.copySeed')" @click="copySeed" />
      </span>
    </div>

    <UEmpty v-if="seen.fields === 0 && seen.skipped === 0" id="no-fields" icon="i-lucide-text-cursor-input" :title="t('panel.noFields')" variant="naked" />

    <template v-for="(section, at) in seen.sections" :key="section.selector ?? ''">
      <UCard
        v-if="several"
        as="section"
        data-form
        :aria-labelledby="'form-' + at"
        :ui="{ header: 'flex min-w-0 items-baseline gap-2 px-3 py-2 sm:px-3', body: 'p-0 sm:p-0' }"
      >
        <template #header>
          <h2 :id="'form-' + at" class="max-w-[60%] shrink-0 truncate text-sm font-semibold text-highlighted" :title="section.name">{{ section.name }}</h2>
          <code v-if="section.selector" class="min-w-0 truncate text-xs text-muted" :title="section.selector">{{ section.selector }}</code>
        </template>
        <FieldTable :section="section" />
      </UCard>
      <FieldTable v-else :section="section" />
    </template>
  </section>
</template>
