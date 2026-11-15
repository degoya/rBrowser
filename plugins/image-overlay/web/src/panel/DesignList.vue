<script setup lang="ts">
// The designs (RB-080-74, on Nuxt UI since RB-080-86): a row per open view with the preview of
// the design it shows, its choice of design (Nearest width, every design by name and size, an
// unusable one marked, a chosen one that is gone kept and marked missing) and what it shows or
// why not; then the folder's designs as a grid of previews with name and size. An empty folder
// is an empty state with its own Add design button; without a trusted project a note says why
// adding cannot work.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { errorText } from '../../../../web-common/panel-ui/src/format';
import { AUTO, designChoices, isMissing, type Design, type OverlayState, type Problem, type ViewRow } from './model';
import type { Previews } from './previews';

const props = defineProps<{
  state: OverlayState;
  views: ViewRow[];
  files: string[];
  library: Design[];
  more: boolean;
  trusted: boolean | null;
  adding: boolean;
  previews: Previews;
}>();
const emit = defineEmits<{ change: [args: Record<string, unknown>]; add: [] }>();
const panel = usePanel();
const t = panel.t;

const problemText = (problem: Problem): string => errorText(panel.strings.value, problem.code, problem.params ?? {});
const designOf = (path: string | undefined): Design | undefined => props.library.find((design) => design.path === path);
const current = (breakpoint: string): string | undefined => props.state.designs[breakpoint];
const missing = (breakpoint: string): boolean => isMissing(props.files, current(breakpoint));

function shown(view: ViewRow): string {
  if (!view.image) return t('panel.design_missing');
  const { path, width, height, scale } = view.image;
  return t('panel.design_shown', { path, width, height, scale });
}

function choose(breakpoint: string, value: unknown): void {
  emit('change', { designs: { [breakpoint]: value === AUTO ? null : String(value) } });
}
</script>

<template>
  <section aria-labelledby="designs-title" class="flex flex-col gap-3">
    <h2 id="designs-title" class="text-sm font-semibold text-highlighted">{{ t('panel.designs') }}</h2>

    <UEmpty v-if="files.length === 0" id="designs-none" icon="i-lucide-image-plus" variant="outline" size="sm" :description="t('panel.designs_none')">
      <template #actions>
        <UButton
          id="add-design-empty"
          icon="i-lucide-image-plus"
          size="sm"
          :label="t('panel.add_design')"
          :disabled="adding"
          :aria-busy="adding ? 'true' : undefined"
          @click="emit('add')"
        />
      </template>
    </UEmpty>
    <UAlert v-if="trusted === false" id="add-untrusted" color="warning" variant="subtle" icon="i-lucide-shield-alert" :description="t('panel.add_untrusted')" :ui="{ description: 'text-xs' }" />

    <p v-if="views.length === 0" id="views-empty" class="text-sm text-muted">{{ t('panel.designs_empty') }}</p>
    <ul v-else id="views" class="flex flex-col gap-2">
      <li v-for="view in views" :key="view.view" class="flex items-start gap-3 rounded-md border border-default p-2" :data-view="view.view">
        <div class="flex h-16 w-12 shrink-0 items-start justify-center overflow-hidden rounded-sm bg-elevated">
          <img v-if="previews.url(designOf(view.image?.path)?.hash)" :src="previews.url(designOf(view.image?.path)?.hash)" alt="" class="w-full" :data-hash="designOf(view.image?.path)?.hash" />
          <UIcon v-else name="i-lucide-image" class="m-auto size-5 text-dimmed" />
        </div>
        <div class="flex min-w-0 flex-1 flex-col gap-1">
          <div class="truncate text-sm font-semibold text-highlighted" :title="view.label" data-name>{{ t('panel.view_row', { label: view.label }) }}</div>
          <USelect
            v-if="view.breakpoint"
            :model-value="current(view.breakpoint) ?? AUTO"
            :items="designChoices(t, files, library, current(view.breakpoint))"
            :data-breakpoint="view.breakpoint"
            :data-missing="missing(view.breakpoint) ? 'true' : undefined"
            :aria-label="t('panel.design_for', { breakpoint: view.breakpoint })"
            :color="missing(view.breakpoint) ? 'warning' : 'primary'"
            :highlight="missing(view.breakpoint)"
            size="sm"
            class="w-full"
            @update:model-value="(value: unknown) => choose(view.breakpoint, value)"
          />
          <UBadge v-else color="neutral" variant="outline" size="sm" class="self-start" :label="t('panel.breakpoint_free')" />
          <p v-if="view.problem" class="text-xs text-error" data-problem>{{ problemText(view.problem) }}</p>
          <p v-else class="truncate text-xs text-muted" :title="shown(view)" data-info>{{ shown(view) }}</p>
        </div>
      </li>
    </ul>

    <template v-if="library.length > 0">
      <h3 id="library-title" class="mt-2 text-sm font-semibold text-highlighted">{{ t('panel.library') }}</h3>
      <ul id="library" aria-labelledby="library-title" class="grid grid-cols-[repeat(auto-fill,minmax(8rem,1fr))] gap-2">
        <li v-for="design in library" :key="design.path" class="flex min-w-0 flex-col overflow-hidden rounded-md border border-default">
          <div class="flex h-20 items-start justify-center overflow-hidden bg-elevated">
            <img
              v-if="previews.url(design.hash)"
              :src="previews.url(design.hash)"
              :alt="t('panel.preview', { name: design.name })"
              :data-hash="design.hash"
              class="w-full"
            />
            <UIcon v-else name="i-lucide-image" class="m-auto size-6 text-dimmed" data-pending />
          </div>
          <div class="flex min-w-0 flex-col gap-0.5 p-2">
            <span class="truncate text-sm font-medium text-highlighted" :title="design.name" data-name>{{ design.name }}</span>
            <span v-if="design.problem" class="text-xs text-error" data-problem>{{ problemText(design.problem) }}</span>
            <span v-else class="truncate text-xs tabular-nums text-muted" data-size>{{ t('panel.size', { width: design.width, height: design.height }) }}</span>
          </div>
        </li>
      </ul>
      <p v-if="more" id="library-more" class="text-xs text-muted">{{ t('panel.library_more', { count: library.length }) }}</p>
    </template>
  </section>
</template>
