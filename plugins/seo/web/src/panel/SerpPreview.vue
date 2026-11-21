<script setup lang="ts">
// The search result of the page (RB-090-95), once per device the setting `seo.serpPreview` names:
// the favicon, the site name and the URL as a breadcrumb, the title and the description - each
// wrapped and cut at the device's pixel limits from the answer, measured in a fixed system
// sans-serif. Under each text its characters and width against the limit, and a note when it was
// cut (the whole text as the tooltip), and why the favicon does not show. Built from the panel's theme tokens, after the search's
// colours (a blue title, a muted URL) - no logo, no brand artwork, no web font.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { sourceLink } from './model';
import { breadcrumbText, canvasMeasure, type Device, faviconNote, fit, type Fitted, FONT_FAMILY, safeFavicon, type Serp, URL_FONT_PX } from './serp';

const props = defineProps<{ serp: Serp; devices: Device[] }>();
const panel = usePanel();
const t = panel.t;
const measure = canvasMeasure();

/** What one device shows. */
interface Result {
  device: Device;
  width: number;
  titlePx: number;
  descriptionPx: number;
  title: Fitted | null;
  description: Fitted | null;
  breadcrumb: Fitted;
}

const results = computed<Result[]>(() =>
  props.devices.map((device) => {
    const limits = props.serp.limits[device];
    const { title, description } = props.serp;
    return {
      device,
      width: limits.width,
      titlePx: limits.titleFontPx,
      descriptionPx: limits.descriptionFontPx,
      title: title ? fit(title.text, limits.width, limits.titleLines, limits.titleFontPx, measure) : null,
      description: description ? fit(description.text, limits.width, limits.descriptionLines, limits.descriptionFontPx, measure) : null,
      breadcrumb: fit(breadcrumbText(props.serp.breadcrumb), limits.width - 40, 1, URL_FONT_PX, measure),
    };
  }),
);
const favicon = computed(() => safeFavicon(props.serp.favicon));
const faviconText = computed(() => faviconNote(t, props.serp.favicon, (code) => panel.error(code)));
const link = computed(() => sourceLink(props.serp.limits.source));
const unverified = computed(() => props.serp.limits.confidence === 'unverified');

const measured = (key: string, value: Fitted): string => t(key, { chars: value.chars, width: value.widthPx, limit: value.limitPx });
const cutNote = (value: Fitted): string => t('panel.serp.cut', { shown: value.shownChars, count: value.chars });
const tooltip = (value: Fitted, source: string): string => `${value.full} (${t('panel.source', { source })})`;

function openSource(): void {
  if (link.value) panel.post({ target: 'host', action: 'open_link', url: link.value });
}
</script>

<template>
  <UCard
    id="serp"
    as="section"
    aria-labelledby="serp-title"
    :ui="{ header: 'flex flex-wrap items-center gap-1.5 px-3 py-2 sm:px-3', body: 'flex flex-col gap-3 p-3 sm:p-3' }"
  >
    <template #header>
      <UIcon name="i-lucide-search" class="size-4 shrink-0 text-muted" />
      <h2 id="serp-title" class="flex-1 text-sm font-semibold text-highlighted">{{ t('panel.serp.title') }}</h2>
      <UTooltip :text="t('panel.serp.checked', { date: serp.limits.checked })">
        <UBadge
          data-confidence
          :color="unverified ? 'warning' : 'neutral'"
          variant="subtle"
          size="sm"
          :icon="unverified ? 'i-lucide-circle-help' : undefined"
          :label="t(`panel.confidence.${serp.limits.confidence}`)"
        />
      </UTooltip>
      <UTooltip v-if="link" :text="t('panel.serp.open_source')">
        <UButton
          data-source
          icon="i-lucide-external-link"
          color="neutral"
          variant="ghost"
          size="xs"
          :aria-label="t('panel.serp.open_source')"
          @click="openSource"
        />
      </UTooltip>
    </template>

    <UAlert v-if="serp.noindex" data-noindex color="warning" variant="subtle" icon="i-lucide-eye-off" :title="t('panel.serp.noindex')" />

    <div class="flex flex-wrap items-start gap-3">
      <div v-for="result in results" :key="result.device" :data-device="result.device" class="flex min-w-0 max-w-full flex-col gap-1">
        <h3 class="text-xs font-semibold text-muted">{{ t(`panel.serp.device.${result.device}`, { width: result.width }) }}</h3>
        <div class="max-w-full overflow-x-auto">
          <div
            data-result
            class="flex flex-col gap-1 rounded-md border border-default bg-default p-3"
            :style="{ width: `${result.width + 26}px`, fontFamily: FONT_FAMILY }"
          >
            <div class="flex items-center gap-2.5">
              <span data-favicon class="flex size-7 shrink-0 items-center justify-center rounded-full border border-default bg-elevated">
                <img v-if="favicon" :src="favicon" alt="" class="size-[18px] object-contain" />
                <UIcon v-else name="i-lucide-globe" class="size-4 text-muted" :title="faviconText" />
              </span>
              <div class="flex min-w-0 flex-col">
                <span data-site class="text-sm text-highlighted">{{ serp.siteName.text }}</span>
                <span data-breadcrumb class="whitespace-nowrap text-muted" :style="{ fontSize: `${URL_FONT_PX}px` }" :title="serp.url">{{
                  result.breadcrumb.text
                }}</span>
              </div>
            </div>
            <p
              v-if="result.title && serp.title"
              data-title
              class="text-info"
              :style="{ fontSize: `${result.titlePx}px`, lineHeight: 1.3 }"
              :title="tooltip(result.title, serp.title.source)"
            >
              <span v-for="(line, index) in result.title.lines" :key="index" class="block whitespace-nowrap">{{ line }}</span>
            </p>
            <p v-else data-title data-missing class="text-sm italic text-muted">{{ t('panel.serp.no_title') }}</p>
            <p
              v-if="result.description && serp.description"
              data-description
              class="text-toned"
              :style="{ fontSize: `${result.descriptionPx}px`, lineHeight: 1.58 }"
              :title="tooltip(result.description, serp.description.source)"
            >
              <span v-for="(line, index) in result.description.lines" :key="index" class="block whitespace-nowrap">{{ line }}</span>
            </p>
            <p v-else data-description data-missing class="text-sm italic text-muted">{{ t('panel.serp.no_description') }}</p>
          </div>
        </div>
        <template v-if="result.title">
          <p data-measure="title" class="text-xs text-muted">{{ measured('panel.serp.title_measure', result.title) }}</p>
          <p v-if="result.title.truncated" data-cut="title" class="text-xs italic text-muted">{{ cutNote(result.title) }}</p>
        </template>
        <template v-if="result.description">
          <p data-measure="description" class="text-xs text-muted">{{ measured('panel.serp.description_measure', result.description) }}</p>
          <p v-if="result.description.truncated" data-cut="description" class="text-xs italic text-muted">{{ cutNote(result.description) }}</p>
        </template>
      </div>
    </div>
    <p v-if="faviconText" data-favicon-note class="flex items-start gap-1.5 text-xs text-muted">
      <UIcon name="i-lucide-info" class="mt-px size-3.5 shrink-0" />
      <span>{{ faviconText }}</span>
    </p>
  </UCard>
</template>
