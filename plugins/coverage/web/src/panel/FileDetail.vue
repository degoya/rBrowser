<script setup lang="ts">
// A file's detail (RB-080-07, on Nuxt UI since RB-080-84), a window of its text at a time: the
// rules (a style sheet) or functions (a script) with the breakpoints using each - nowhere in the
// warning's colour and words -, and a style sheet's source coloured by the one highlighter with
// the parts no view used marked (tinted and underlined, said in the legend, not by colour alone).
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { markedTokens } from '../../../../web-common/panel-ui/src/code/highlight';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import { type CoverageDetail, type CoverageSummary, usedAnywhere, viewName, windowRanges } from './model';

const props = defineProps<{ detail: CoverageDetail; summary: CoverageSummary | null }>();
defineEmits<{ back: []; more: []; close: [] }>();
const { t, settings } = usePanel();

const to = computed(() => Math.min(props.detail.offset + props.detail.length, props.detail.total));

/** The breakpoints of `views`, by name. */
function names(views: number[]): string {
  return (props.summary?.views ?? [])
    .filter((view) => views.includes(view.view))
    .map(viewName)
    .join(', ');
}

const items = computed(() => {
  const rules = (props.detail.rules ?? []).map((rule) => ({
    name: rule.prelude,
    nowhere: rule.usedIn.length === 0,
    where: rule.usedIn.length ? t('panel.usedIn', { breakpoints: names(rule.usedIn) }) : t('panel.unusedEverywhere'),
  }));
  const functions = (props.detail.functions ?? []).map((fn) => ({
    name: fn.name || t('panel.anonymous'),
    nowhere: fn.executedIn.length === 0,
    where: fn.executedIn.length ? t('panel.executedIn', { breakpoints: names(fn.executedIn) }) : t('panel.neverRan'),
  }));
  return [...rules, ...functions];
});

/** The source window cut into coloured tokens, the used parts marked. */
const source = computed(() => {
  void settings.value;
  const text = props.detail.text;
  if (typeof text !== 'string') return null;
  const used = windowRanges(props.detail.offset, text.length, usedAnywhere(props.detail.views));
  return markedTokens(text, props.detail.kind === 'css' ? 'css' : 'js', used);
});
</script>

<template>
  <UCard id="detail" as="section" aria-labelledby="detail-title" :ui="{ header: 'flex flex-wrap items-center gap-2 px-3 py-2 sm:px-3', body: 'flex flex-col gap-3 p-3 sm:p-3' }">
    <template #header>
      <UTooltip :text="detail.url" :content="{ side: 'top', align: 'start' }">
        <h2 id="detail-title" class="min-w-0 flex-1 truncate font-mono text-xs font-semibold text-highlighted">{{ detail.url }}</h2>
      </UTooltip>
      <span id="window" class="text-xs text-muted">{{ t('panel.window', { from: detail.offset, to, total: detail.total }) }}</span>
      <span class="flex items-center gap-1">
        <IconAction id="back" icon="i-lucide-chevron-left" :label="t('panel.back')" :disabled="detail.offset === 0" @click="$emit('back')" />
        <IconAction id="more" icon="i-lucide-chevron-right" :label="t('panel.more')" :disabled="detail.next === null" @click="$emit('more')" />
        <IconAction id="close" icon="i-lucide-x" :label="t('panel.close')" @click="$emit('close')" />
      </span>
    </template>

    <section class="flex flex-col gap-1">
      <h3 id="items-title" class="text-xs font-semibold uppercase text-toned">{{ t(detail.kind === 'css' ? 'panel.rules' : 'panel.functions') }}</h3>
      <ul id="items" class="max-h-56 divide-y divide-default overflow-auto rounded-md border border-default">
        <li v-for="(item, index) in items" :key="index" class="flex flex-wrap items-baseline justify-between gap-2 px-2 py-1">
          <span class="min-w-0 break-all font-mono text-xs text-highlighted">{{ item.name }}</span>
          <span data-where :data-tone="item.nowhere ? 'warn' : undefined" class="inline-flex items-center gap-1 text-xs" :class="item.nowhere ? 'text-warning' : 'text-muted'">
            <UIcon v-if="item.nowhere" name="i-lucide-triangle-alert" class="size-3.5" />{{ item.where }}
          </span>
        </li>
      </ul>
    </section>

    <section class="flex flex-col gap-1">
      <h3 id="source-title" class="text-xs font-semibold uppercase text-toned">{{ t('panel.source') }}</h3>
      <p v-if="!source" id="no-source" class="text-sm text-muted">{{ t('panel.noSource') }}</p>
      <template v-else>
        <p id="legend" class="flex flex-wrap items-center gap-3 text-xs text-muted">
          <span class="inline-flex items-center gap-1"><span class="cov-used rounded-sm px-1">abc</span>{{ t('panel.legendUsed') }}</span>
          <span class="inline-flex items-center gap-1"><span class="cov-unused rounded-sm px-1">abc</span>{{ t('panel.legendUnused') }}</span>
        </p>
        <pre
          id="source"
          tabindex="0"
          class="m-0 max-h-96 overflow-auto rounded-md border border-default bg-elevated/50 p-2 font-mono text-xs whitespace-pre-wrap break-all outline-none focus-visible:ring-2 focus-visible:ring-primary"
        ><span v-for="(piece, index) in source" :key="index" :class="[piece.marked ? 'cov-used' : 'cov-unused', piece.kind === 'plain' ? '' : 'hl-' + piece.kind]">{{ piece.text }}</span></pre>
      </template>
    </section>
  </UCard>
</template>
