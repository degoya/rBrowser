<script setup lang="ts">
// One platform's card (RB-080-85, RB-080-99): its name and the card its rule picks as a badge;
// the preview in that layout, then the reason in words with how sure the rule is and a link to its
// source.
// A sure rule keeps the other variant behind a toggle that shows it below, the chosen one stays;
// an unsure rule (none published, or the image's size unknown) shows both, marked.
import { computed, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import CardPreview from './CardPreview.vue';
import { type Card, type Layout, otherLayout, type Preview, reasonText, shownLayouts, sourceLink } from './model';

const props = defineProps<{ preview: Preview; card: Card }>();
const panel = usePanel();
const t = panel.t;

const open = ref(false);
const shown = computed(() => shownLayouts(props.card));
const other = computed(() => otherLayout(props.card));
const link = computed(() => sourceLink(props.card.source));
const layoutWord = (layout: Layout): string => t(`panel.layout.${layout}`);

function openSource(): void {
  if (link.value) panel.post({ target: 'host', action: 'open_link', url: link.value });
}
</script>

<template>
  <UCard
    as="article"
    :data-platform="card.id"
    :data-unsure="card.unsure ? '' : undefined"
    tabindex="0"
    :aria-labelledby="'card-' + card.id"
    class="focus-visible:outline-2 focus-visible:outline-primary"
    :ui="{ header: 'flex flex-wrap items-center gap-1.5 px-3 py-2 sm:px-3', body: 'flex flex-col gap-2 p-3 sm:p-3' }"
  >
    <template #header>
      <h2 :id="'card-' + card.id" class="flex-1 text-sm font-semibold text-highlighted">{{ card.name }}</h2>
      <UBadge v-if="!card.unsure" data-layout color="primary" variant="subtle" :label="layoutWord(card.layout)" />
    </template>

    <div v-for="layout in shown" :key="layout" data-shown class="flex flex-col gap-1">
      <UBadge v-if="card.unsure" data-layout color="warning" variant="outline" :label="layoutWord(layout)" class="self-start" />
      <CardPreview :preview="preview" :card="card" :layout="layout" />
    </div>

    <div class="flex flex-wrap items-center gap-1.5">
      <p data-reason class="flex min-w-0 flex-[1_1_12rem] items-start gap-1.5 text-xs text-muted">
        <UIcon name="i-lucide-info" class="mt-px size-3.5 shrink-0" />
        <span>{{ reasonText(t, card) }}</span>
      </p>
      <UBadge
        data-confidence
        :color="card.confidence === 'unverified' ? 'warning' : 'neutral'"
        variant="subtle"
        size="sm"
        :icon="card.confidence === 'unverified' ? 'i-lucide-circle-help' : undefined"
        :label="t(`panel.confidence.${card.confidence}`)"
      />
      <UTooltip v-if="link" :text="t('panel.open_source')">
        <UButton
          data-source
          icon="i-lucide-external-link"
          color="neutral"
          variant="ghost"
          size="xs"
          :aria-label="t('panel.open_source')"
          @click="openSource"
        />
      </UTooltip>
    </div>

    <template v-if="other">
      <UButton
        data-toggle
        class="self-start"
        size="xs"
        color="neutral"
        variant="soft"
        :icon="open ? 'i-lucide-eye-off' : 'i-lucide-eye'"
        :aria-expanded="open ? 'true' : 'false'"
        :label="open ? t('panel.hide_other') : t(`panel.show_other.${other}`)"
        @click="open = !open"
      />
      <div v-if="open" data-other class="flex flex-col gap-1 border-t border-dashed border-default pt-2">
        <p class="text-xs text-muted">{{ t('panel.other', { layout: layoutWord(other) }) }}</p>
        <CardPreview :preview="preview" :card="card" :layout="other" />
      </div>
    </template>
  </UCard>
</template>
