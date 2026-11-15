<script setup lang="ts">
// The breakpoint analyser's wizard (RB-010-13), the route `analyzer` of the pages window: the
// boundaries found with their stylesheets, the proposed views to tick, a preview of the grid,
// then one button that writes `config.json`. With breakpoints already configured only the new
// ones are listed. The logic is `wizard.ts`.
import { computed, onBeforeUnmount, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { IpcClient } from '../ipc/client';
import type { AnalyzerMode } from '../ipc/protocol';
import { createWizard, hintKind, parseUrls } from './wizard';

const props = defineProps<{ client: IpcClient }>();
const { t } = useI18n();
const wizard = createWizard(props.client);
onBeforeUnmount(() => wizard.stop());

const modes: AnalyzerMode[] = ['all', 'simplified', 'edges'];
const mode = ref<AnalyzerMode | ''>('');
const urlText = ref('');
const proposal = wizard.proposal;
const busy = computed(() => wizard.phase.value === 'running' || wizard.phase.value === 'applying');
const scale = computed(() => {
  const widest = Math.max(1, ...wizard.preview.value.cells.map((cell) => cell.width));
  return 120 / widest;
});

function analyze(): void {
  void wizard.analyze(parseUrls(urlText.value), mode.value === '' ? undefined : mode.value);
}
</script>

<template>
  <section class="flex flex-col gap-4 p-4 text-default" data-testid="analyzer-wizard">
    <header>
      <h1 class="text-lg font-semibold">{{ t('analyzer.wizard.title') }}</h1>
      <p class="text-sm text-muted">{{ t('analyzer.wizard.intro') }}</p>
    </header>

    <form class="flex flex-wrap items-end gap-3" @submit.prevent="analyze">
      <label class="flex flex-col gap-1 text-sm">
        {{ t('analyzer.wizard.mode') }}
        <select v-model="mode" class="rounded border border-default bg-default px-2 py-1" data-testid="mode">
          <option value="">{{ t('analyzer.wizard.mode_default') }}</option>
          <option v-for="m in modes" :key="m" :value="m">{{ t(`analyzer.wizard.modes.${m}`) }}</option>
        </select>
      </label>
      <label class="flex min-w-64 flex-1 flex-col gap-1 text-sm">
        {{ t('analyzer.wizard.urls') }}
        <textarea v-model="urlText" rows="2" class="rounded border border-default bg-default px-2 py-1" data-testid="urls" />
      </label>
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
      <details class="text-sm">
        <summary>{{ t('analyzer.wizard.sources') }}</summary>
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
      </details>
      <ul v-if="proposal.hints?.length" class="text-sm text-muted" data-testid="hints">
        <li v-for="(hint, index) in proposal.hints" :key="index">
          {{ t(`analyzer.wizard.hints.${hintKind(hint)}`, { path: hint.path, name: hint.params.name ?? '' }) }}
        </li>
      </ul>

      <p v-if="proposal.existing" class="text-sm" data-testid="diff-note">
        {{ wizard.rows.value.length === 0 ? t('analyzer.wizard.nothing_new') : t('analyzer.wizard.diff') }}
      </p>
      <fieldset v-if="wizard.rows.value.length > 0" class="flex flex-col gap-1 text-sm">
        <legend>{{ t('analyzer.wizard.proposed') }}</legend>
        <label v-for="row in wizard.rows.value" :key="row.id" class="flex items-center gap-2" :data-testid="`row-${row.id}`">
          <input v-model="row.selected" type="checkbox" />
          {{ t('analyzer.wizard.view', { label: row.label, width: String(row.width), height: String(row.height) }) }}
        </label>
      </fieldset>
      <label v-if="proposal.existing && wizard.rows.value.length > 0" class="flex items-center gap-2 text-sm">
        <input v-model="wizard.replace.value" type="checkbox" data-testid="replace" />
        {{ t('analyzer.wizard.replace') }}
      </label>

      <figure v-if="wizard.chosen.value.length > 0" class="flex flex-col gap-1 text-sm">
        <figcaption>{{ t('analyzer.wizard.preview') }}</figcaption>
        <div
          class="grid items-start gap-2"
          :style="{ gridTemplateColumns: `repeat(${wizard.preview.value.columns}, max-content)` }"
          data-testid="preview"
        >
          <div
            v-for="cell in wizard.preview.value.cells"
            :key="cell.id"
            class="flex items-center justify-center rounded border border-default text-xs"
            :style="{ width: `${Math.round(cell.width * scale)}px`, height: `${Math.round(cell.height * scale)}px` }"
            :title="t('analyzer.wizard.view', { label: cell.label, width: String(cell.width), height: String(cell.height) })"
          >
            {{ cell.label }}
          </div>
        </div>
      </figure>

      <div class="flex items-center gap-3">
        <UButton :disabled="busy || wizard.chosen.value.length === 0" data-testid="apply" @click="wizard.apply()">
          {{ t('analyzer.wizard.apply', { count: String(wizard.chosen.value.length) }) }}
        </UButton>
        <span class="text-xs text-muted">{{ t('analyzer.wizard.nothing_written') }}</span>
      </div>
    </template>
  </section>
</template>
