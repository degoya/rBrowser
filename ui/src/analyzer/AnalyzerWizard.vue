<script setup lang="ts">
// The breakpoint analyser's wizard (RB-010-13), the route `analyzer` of the pages window: the
// boundaries found with their stylesheets, the proposed views to tick, a preview of the grid,
// then one button that writes `config.json`. With breakpoints already configured only the new
// ones are listed. Without a project the analysis still runs - it only reads the pages - but
// the button stays off with its reason, and the top offers the recent projects (RB-090-43).
// The logic is `wizard.ts`, the preview's geometry `preview.ts`. Every control is Nuxt UI
// (RB-080-88): the views to write and "replace" are `UCheckbox`es, the stylesheets and boundaries
// found a `UCollapsible`.
import { computed, onBeforeUnmount, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UCheckbox from '@nuxt/ui/components/Checkbox.vue';
import UCollapsible from '@nuxt/ui/components/Collapsible.vue';
import UFormField from '@nuxt/ui/components/FormField.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import UTextarea from '@nuxt/ui/components/Textarea.vue';
import { ICONS } from '../icons';
import type { IpcClient } from '../ipc/client';
import type { AnalyzerMode } from '../ipc/protocol';
import AnalyzerPreview from './AnalyzerPreview.vue';
import NoProjectNotice from './NoProjectNotice.vue';
import { createWizard, hintKind, parseUrls } from './wizard';

const props = defineProps<{
  client: IpcClient;
  /** Whether a project is open; `null` (or absent) while the core's state is not known yet. */
  projectOpen?: boolean | null;
}>();
const { t } = useI18n();
const wizard = createWizard(props.client);
onBeforeUnmount(() => wizard.stop());

/** The mode of the settings: the request names none. */
const DEFAULT_MODE = 'default';
const modes: AnalyzerMode[] = ['all', 'simplified', 'edges'];
const modeItems = computed(() => [
  { label: t('analyzer.wizard.mode_default'), value: DEFAULT_MODE },
  ...modes.map((value) => ({ label: t(`analyzer.wizard.modes.${value}`), value })),
]);
const mode = ref<AnalyzerMode | typeof DEFAULT_MODE>(DEFAULT_MODE);
const urlText = ref('');
const sourcesOpen = ref(false);
const proposal = wizard.proposal;
const busy = computed(() => wizard.phase.value === 'running' || wizard.phase.value === 'applying');
const noProject = computed(() => props.projectOpen === false);
const canApply = computed(() => !noProject.value && !busy.value && wizard.chosen.value.length > 0);

function analyze(): void {
  void wizard.analyze(parseUrls(urlText.value), mode.value === DEFAULT_MODE ? undefined : mode.value);
}

function apply(): void {
  if (canApply.value) {
    void wizard.apply();
  }
}
</script>

<template>
  <section class="flex flex-col gap-4 p-4 text-default" data-testid="analyzer-wizard">
    <header>
      <h1 class="text-lg font-semibold">{{ t('analyzer.wizard.title') }}</h1>
      <p class="text-sm text-muted">{{ t('analyzer.wizard.intro') }}</p>
    </header>

    <NoProjectNotice v-if="noProject" :client="client" />

    <form class="flex flex-wrap items-end gap-3" @submit.prevent="analyze">
      <UFormField :label="t('analyzer.wizard.mode')">
        <USelect v-model="mode" :items="modeItems" class="w-56" data-testid="mode" />
      </UFormField>
      <UFormField :label="t('analyzer.wizard.urls')" class="min-w-64 flex-1">
        <UTextarea v-model="urlText" :rows="2" class="w-full" data-testid="urls" />
      </UFormField>
      <UButton type="submit" :loading="wizard.phase.value === 'running'" :disabled="busy" data-testid="run">
        {{ t('analyzer.wizard.analyze') }}
      </UButton>
    </form>

    <p v-if="wizard.phase.value === 'running'" role="status" class="text-sm">{{ t('analyzer.wizard.running') }}</p>
    <p v-if="wizard.message.value" role="status" class="text-sm" data-testid="message">
      {{ t(wizard.message.value.key, wizard.message.value.args) }}
    </p>

    <template v-if="proposal">
      <p class="text-sm" data-testid="summary">
        {{ t('analyzer.wizard.summary', { boundaries: String(proposal.detected.length), sheets: String(proposal.sources.length) }) }}
      </p>
      <UCollapsible v-model:open="sourcesOpen" class="flex flex-col gap-1 text-sm" data-testid="sources">
        <UButton
          size="sm"
          variant="ghost"
          color="neutral"
          class="self-start"
          :trailing-icon="ICONS.checkDetails"
          :ui="{ trailingIcon: sourcesOpen ? 'rotate-180 transition-transform' : 'transition-transform' }"
          data-testid="sources-toggle"
        >
          {{ t('analyzer.wizard.sources') }}
        </UButton>
        <template #content>
          <ul>
            <li v-for="source in proposal.sources" :key="source.url">
              {{ t('analyzer.wizard.source', { url: source.url, count: String(source.count) }) }}
            </li>
          </ul>
          <ul>
            <li v-for="found in proposal.detected" :key="`${found.kind}${found.width}`">
              {{ t(`analyzer.wizard.boundary.${found.kind}`, { width: String(found.width), label: found.label ?? '', count: String(found.count) }) }}
              <span v-if="found.source" class="text-muted" :data-testid="`declared-${found.width}`">
                {{ t(found.heuristic ? 'analyzer.wizard.declared_heuristic' : 'analyzer.wizard.declared', { source: found.source }) }}
              </span>
            </li>
          </ul>
        </template>
      </UCollapsible>
      <ul v-if="proposal.hints?.length" class="text-sm text-muted" data-testid="hints">
        <li v-for="(hint, index) in proposal.hints" :key="index">
          {{ t(`analyzer.wizard.hints.${hintKind(hint)}`, { path: hint.path, name: hint.params.name ?? '' }) }}
        </li>
      </ul>

      <p v-if="proposal.existing" class="text-sm" data-testid="diff-note">
        {{ wizard.rows.value.length === 0 ? t('analyzer.wizard.nothing_new') : t('analyzer.wizard.diff') }}
      </p>
      <fieldset v-if="wizard.rows.value.length > 0" class="flex flex-col gap-1 text-sm">
        <legend class="mb-1 font-medium">{{ t('analyzer.wizard.proposed') }}</legend>
        <UCheckbox
          v-for="row in wizard.rows.value"
          :key="row.id"
          v-model="row.selected"
          :label="t('analyzer.wizard.view', { label: row.label, width: String(row.width), height: String(row.height) })"
          :ui="{ label: 'font-normal' }"
          :data-testid="`row-${row.id}`"
        />
      </fieldset>
      <UCheckbox
        v-if="proposal.existing && wizard.rows.value.length > 0"
        v-model="wizard.replace.value"
        class="text-sm"
        :label="t('analyzer.wizard.replace')"
        :ui="{ label: 'font-normal' }"
        data-testid="replace"
      />

      <AnalyzerPreview
        v-if="wizard.chosen.value.length > 0"
        :grid="wizard.preview.value"
        :appended="wizard.appended.value"
      />

      <div class="flex flex-wrap items-center gap-3">
        <UButton
          :disabled="!canApply"
          :aria-describedby="noProject ? 'analyzer-apply-reason' : undefined"
          data-testid="apply"
          @click="apply"
        >
          {{ t('analyzer.wizard.apply', { count: String(wizard.chosen.value.length) }) }}
        </UButton>
        <span v-if="noProject" id="analyzer-apply-reason" class="text-xs text-muted" data-testid="apply-reason">
          {{ t('analyzer.wizard.apply_needs_project') }}
        </span>
        <span v-else class="text-xs text-muted">{{ t('analyzer.wizard.nothing_written') }}</span>
      </div>
    </template>
  </section>
</template>
