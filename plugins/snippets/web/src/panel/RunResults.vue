<script setup lang="ts">
// The recent runs (RB-070-04, on Nuxt UI since RB-080-84): a card per run, a line per outcome with
// the views it came from - a value coloured by the one highlighter (JSON or a function; a string
// is text), a failure in words -, its stack on request, coloured as a stack.
import { ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import CodeBlock from '../../../../web-common/panel-ui/src/code/CodeBlock.vue';
import { type RunRecord, type ViewEntry, failureText } from './model';

const props = defineProps<{ runs: RunRecord[]; views: ViewEntry[] }>();
const panel = usePanel();
const t = panel.t;
const stacks = ref<Record<string, boolean>>({});

function viewNames(ids: number[]): string {
  const byId = new Map(props.views.map((view) => [view.id, view.label]));
  return ids.map((id) => byId.get(id) ?? String(id)).join(', ');
}
</script>

<template>
  <section class="flex flex-col gap-2" aria-labelledby="results-title">
    <h2 id="results-title" class="flex items-center gap-1.5 text-xs font-semibold uppercase text-toned">
      <UIcon name="i-lucide-history" class="size-4" />
      <span>{{ t('panel.results') }}</span>
    </h2>
    <ol id="results" aria-live="polite" class="flex flex-col gap-2">
      <li v-for="record in runs" :key="record.id" data-run>
        <UCard :ui="{ header: 'px-3 py-1.5 sm:px-3', body: 'flex flex-col gap-2 p-3 sm:p-3' }">
          <template #header>
            <h3 data-title class="truncate text-sm font-semibold text-highlighted">{{ record.title }}</h3>
          </template>
          <div v-for="(result, index) in record.results" :key="index" data-outcome :data-ok="result.ok" class="flex flex-col gap-1">
            <div class="flex flex-wrap items-center gap-1.5">
              <UBadge
                :color="result.ok ? 'success' : 'error'"
                variant="subtle"
                size="sm"
                :icon="result.ok ? 'i-lucide-circle-check' : 'i-lucide-circle-x'"
                :label="t(result.ok ? 'panel.ok' : 'panel.failed')"
              />
              <UBadge v-if="result.timedOut" data-timed-out color="warning" variant="subtle" size="sm" icon="i-lucide-timer-off" :label="t('panel.timedOut')" />
              <span data-views class="text-xs text-muted">{{ t('panel.viewList', { views: viewNames(result.views) }) }}</span>
            </div>
            <CodeBlock v-if="result.ok" data-text :text="result.text" :lang="highlighter().valueLang(result.type, result.text)" :note="t('panel.plainLarge')" />
            <p v-else data-text class="text-sm break-words text-error">{{ failureText(panel.strings.value, result.text) }}</p>
            <template v-if="result.stack">
              <div>
                <UButton
                  data-stack-toggle
                  size="xs"
                  variant="link"
                  color="neutral"
                  :icon="stacks[record.id + ':' + index] ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'"
                  :label="t('panel.stack')"
                  :aria-expanded="Boolean(stacks[record.id + ':' + index])"
                  @click="stacks[record.id + ':' + index] = !stacks[record.id + ':' + index]"
                />
              </div>
              <CodeBlock v-if="stacks[record.id + ':' + index]" data-stack :text="result.stack" lang="stack" />
            </template>
          </div>
        </UCard>
      </li>
    </ol>
  </section>
</template>
