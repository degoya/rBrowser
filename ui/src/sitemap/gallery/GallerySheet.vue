<script setup lang="ts">
// The contact sheet of the sitemap gallery page (RB-090-86): a row per page, a column per
// breakpoint, each shot a thumbnail as wide as `sitemap-capture.thumbnailWidth` that opens the
// large view. A plain table: a grid of images by page and breakpoint, which UTable's data model
// adds nothing to; every control in it is Nuxt UI.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import type { GalleryShot, SitemapGalleryPage } from '../../ipc/protocol';
import ExternalLink from '../../links/ExternalLink.vue';
import { labelOf, percentOf, problemsOf, shownBreakpoints, shownPages, type GalleryFilter, type ShotRef } from './gallery';

const props = defineProps<{ run: SitemapGalleryPage; filter: GalleryFilter }>();
const emit = defineEmits<{ open: [shot: ShotRef] }>();
const { t } = useI18n();

const breakpoints = computed(() => shownBreakpoints(props.run, props.filter));
const rows = computed(() => shownPages(props.run, props.filter));
const width = computed(() => `${props.run.thumbnailWidth}px`);

function shotOf(shots: Record<string, GalleryShot>, name: string): GalleryShot | null {
  return shots[name] ?? null;
}
</script>

<template>
  <div class="overflow-x-auto rounded-md border border-default" data-testid="gallery-sheet">
    <p v-if="rows.length === 0" class="p-4 text-sm text-muted" data-testid="gallery-empty">
      {{ t('sitemap.galleryPage.empty') }}
    </p>
    <table v-else class="w-full border-collapse text-sm">
      <thead class="bg-elevated/50">
        <tr>
          <th scope="col" class="px-3 py-2 text-left font-medium text-highlighted">{{ t('sitemap.galleryPage.column.page') }}</th>
          <th v-for="bp in breakpoints" :key="bp.name" scope="col" class="px-3 py-2 text-left font-medium text-highlighted" :data-testid="`column-${bp.name}`">
            {{ labelOf(bp) }}
            <span class="ml-1 text-xs font-normal text-muted">{{ t('sitemap.galleryPage.size', { width: bp.width, height: bp.height }) }}</span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="{ page, index } in rows" :key="index" class="border-t border-default align-top" :data-testid="`row-${index}`">
          <th scope="row" class="max-w-64 px-3 py-2 text-left font-normal break-all">
            <ExternalLink :href="page.url" class="text-primary">{{ page.url }}</ExternalLink>
            <UBadge v-if="problemsOf(page).length > 0" color="error" variant="subtle" size="sm" class="ml-2" :data-testid="`problem-${index}`">
              {{ page.status !== undefined && page.status >= 400 ? page.status : t('sitemap.galleryPage.status.problems') }}
            </UBadge>
          </th>
          <td v-for="bp in breakpoints" :key="bp.name" class="px-3 py-2" :data-testid="`cell-${index}-${bp.name}`">
            <template v-if="shotOf(page.shots, bp.name)?.image">
              <UButton
                variant="ghost"
                color="neutral"
                class="block p-0"
                :aria-label="t('sitemap.galleryPage.shot.open', { url: page.url, breakpoint: labelOf(bp) })"
                :data-testid="`shot-${index}-${bp.name}`"
                @click="emit('open', { page: index, breakpoint: bp.name })"
              >
                <img
                  :src="shotOf(page.shots, bp.name)?.image"
                  :alt="t('sitemap.galleryPage.shot.alt', { url: page.url, breakpoint: labelOf(bp) })"
                  :style="{ width }"
                  class="h-auto cursor-zoom-in rounded-sm border border-default"
                  loading="lazy"
                />
              </UButton>
              <UBadge
                v-if="shotOf(page.shots, bp.name)?.compare"
                :color="shotOf(page.shots, bp.name)?.compare?.status === 'ok' ? 'success' : 'warning'"
                variant="subtle"
                size="sm"
                class="mt-1"
              >
                {{
                  t('sitemap.galleryPage.compare', {
                    status: shotOf(page.shots, bp.name)?.compare?.status ?? '',
                    percent: percentOf(shotOf(page.shots, bp.name)?.compare?.ratio ?? 0),
                  })
                }}
              </UBadge>
            </template>
            <UBadge v-else-if="shotOf(page.shots, bp.name)?.state === 'failed'" color="error" variant="subtle" size="sm">
              {{ t('sitemap.galleryPage.shot.failed', { error: shotOf(page.shots, bp.name)?.error ?? '' }) }}
            </UBadge>
            <span v-else class="text-muted">{{ t('sitemap.galleryPage.shot.pending') }}</span>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
