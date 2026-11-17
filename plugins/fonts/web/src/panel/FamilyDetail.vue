<script setup lang="ts">
// The selected family in the Fonts panel (RB-080-83): its facts as badges, then sections that
// keep their open state from family to family - the preview per face in a sample text and size
// of the user's choice (from the settings, changeable for the moment), the faces with their
// state per view, the use per view (with what Chromium drew, when a view could tell), and the
// family's findings, each with its severity as an icon and a word.
import { computed, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Answer, type Family, faceStates, findingsFor, message, renderedKnown, viewLabel } from './model';
import type { FontPreviews } from './preview';

const props = defineProps<{ answer: Answer; family: Family; previews: FontPreviews }>();
const open = defineModel<string[]>('open', { required: true });
const { t, strings, settings } = usePanel();

const SIZES = [12, 16, 20, 24, 32, 48, 64, 96];
const FACE_TONE: Record<string, 'success' | 'error' | 'neutral'> = { loaded: 'success', error: 'error' };
const DRAWN_TONE: Record<string, 'success' | 'warning' | 'neutral'> = { ok: 'success', fallback: 'warning', glyphs: 'warning' };
const SEVERITY: Record<string, { icon: string; class: string }> = {
  error: { icon: 'i-lucide-circle-x', class: 'text-error' },
  warning: { icon: 'i-lucide-triangle-alert', class: 'text-warning' },
};

const text = ref('');
const size = ref(24);
watch(
  settings,
  (now) => {
    text.value = String(now['fonts.previewText'] ?? '') || text.value || 'Aa Bb Cc 0123';
    const px = Number(now['fonts.previewSizePx']);
    if (Number.isFinite(px) && px > 0) size.value = px;
  },
  { immediate: true },
);
const sizes = computed(() => [...new Set([...SIZES, size.value])].sort((a, b) => a - b).map((px) => ({ label: t('panel.preview.sizePx', { px }), value: px })));

const findings = computed(() => findingsFor(props.answer, props.family.family));
const drawn = computed(() => renderedKnown(props.family));
const faces = computed(() => (props.family.webFont ? props.family.faces : [undefined]));

const sections = computed(() => [
  { label: t('panel.preview.title'), value: 'preview', icon: 'i-lucide-eye' },
  ...(props.family.faces.length ? [{ label: t('panel.faces'), value: 'faces', icon: 'i-lucide-file-text' }] : []),
  ...(props.family.usage.length ? [{ label: t('panel.usage'), value: 'usage', icon: 'i-lucide-monitor' }] : []),
  ...(findings.value.length ? [{ label: t('panel.findings'), value: 'findings', icon: 'i-lucide-triangle-alert' }] : []),
]);

const faceColumns = computed(() => [
  { accessorKey: 'weight', header: t('panel.weight') },
  { accessorKey: 'style', header: t('panel.style') },
  { accessorKey: 'display', header: t('panel.display') },
  { id: 'status', header: t('panel.status') },
  { id: 'file', header: t('panel.file') },
]);
const usageColumns = computed(() => [
  { id: 'view', header: t('panel.view') },
  { accessorKey: 'elements', header: t('panel.elements'), meta: { class: { th: 'text-right', td: 'text-right tabular-nums' } } },
  { id: 'weights', header: t('panel.weights') },
  { id: 'sizes', header: t('panel.sizes') },
  ...(drawn.value ? [{ id: 'rendered', header: t('panel.rendered') }] : []),
]);

const faceLabel = (face: { weight: string; style: string }): string => `${face.weight} ${face.style}`;
</script>

<template>
  <section id="detail" aria-live="polite" class="min-w-0">
    <h2 class="text-base font-semibold text-highlighted">{{ family.family }}</h2>
    <div class="mt-1 flex flex-wrap gap-1" data-facts>
      <UBadge color="neutral" variant="subtle" size="sm" icon="i-lucide-globe" :label="t(`panel.provider.${family.provider}`)" />
      <UBadge v-if="family.host" color="neutral" variant="outline" size="sm" :label="family.host" />
      <UBadge v-for="format in family.formats" :key="format" color="neutral" variant="outline" size="sm" :label="format" />
      <UBadge v-if="family.variable" color="info" variant="subtle" size="sm" :label="t('panel.variable')" />
      <UBadge v-if="family.fileKb !== null" color="neutral" variant="outline" size="sm" :label="t('panel.kb', { kb: family.fileKb })" />
    </div>

    <UAccordion v-model="open" type="multiple" :items="sections" :unmount-on-hide="false" class="mt-2">
      <template #body="{ item }">
        <div v-if="item.value === 'preview'" data-section="preview" class="flex flex-col gap-2">
          <div class="flex flex-wrap items-end gap-2">
            <UFormField :label="t('panel.preview.text')" class="min-w-0 flex-1">
              <UInput id="preview-text" v-model="text" size="sm" class="w-full" :maxlength="200" />
            </UFormField>
            <UFormField :label="t('panel.preview.size')">
              <USelect id="preview-size" v-model="size" :items="sizes" size="sm" class="w-28" />
            </UFormField>
          </div>
          <p v-if="!family.webFont" class="text-xs text-muted">{{ t('panel.preview.installed') }}</p>
          <div v-for="face in faces" :key="face ? faceLabel(face) + face.src : 'name'" data-face-preview class="rounded-md border border-default p-2">
            <div v-if="face" class="text-xs text-muted">{{ faceLabel(face) }}</div>
            <template v-for="state in [previews.state(family, face)]" :key="String(state?.kind)">
              <p v-if="state?.kind === 'ready'" class="break-words leading-tight" :style="{ fontFamily: state.css, fontSize: `${size}px`, fontWeight: face?.weight, fontStyle: face?.style }">
                {{ text }}
              </p>
              <p v-else-if="state?.kind === 'missing'" data-reason class="text-sm text-muted">
                {{ t('panel.preview.' + state.reason, { kb: state.kb ?? '' }) }}
              </p>
              <p v-else class="flex items-center gap-1.5 text-sm text-muted">
                <UIcon name="i-lucide-loader-circle" class="size-4 animate-spin" />{{ t('panel.preview.loading') }}
              </p>
            </template>
          </div>
        </div>

        <UTable v-else-if="item.value === 'faces'" data-section="faces" :data="family.faces" :columns="faceColumns" class="text-sm">
          <template #status-cell="{ row }">
            <div class="flex flex-wrap gap-1">
              <UBadge
                v-for="entry in faceStates(answer, row.original)"
                :key="entry.state"
                :color="FACE_TONE[entry.state] ?? 'neutral'"
                variant="subtle"
                size="sm"
                :label="entry.views.length ? t('panel.stateIn', { state: t(`panel.state.${entry.state}`), views: entry.views.join(', ') }) : t(`panel.state.${entry.state}`)"
              />
            </div>
          </template>
          <template #file-cell="{ row }">
            <span>{{ [row.original.format, row.original.kb !== null ? t('panel.kb', { kb: row.original.kb }) : ''].filter(Boolean).join(' · ') }}</span>
            <UBadge v-if="row.original.preloaded" class="ml-1" color="info" variant="subtle" size="sm" :label="t('panel.preloaded')" />
          </template>
        </UTable>

        <UTable v-else-if="item.value === 'usage'" data-section="usage" :data="family.usage" :columns="usageColumns" class="text-sm">
          <template #view-cell="{ row }">
            <span class="font-medium">{{ viewLabel(answer, row.original.view) }}</span>
          </template>
          <template #weights-cell="{ row }">{{ row.original.weights.join(', ') }}</template>
          <template #sizes-cell="{ row }">{{ row.original.sizes.join(', ') }}</template>
          <template #rendered-cell="{ row }">
            <template v-for="entry in family.rendered.filter((one) => one.view === row.original.view)" :key="entry.view">
              <UBadge :color="DRAWN_TONE[entry.state] ?? 'neutral'" variant="subtle" size="sm" :label="t(`panel.drawn.${entry.state}`, { fonts: entry.as.join(', ') })" />
            </template>
          </template>
        </UTable>

        <ul v-else-if="item.value === 'findings'" data-section="findings" class="flex flex-col gap-1">
          <li v-for="(finding, index) in findings" :key="index" class="flex items-start gap-1.5 text-sm" :class="`sev-${finding.severity}`">
            <UIcon :name="SEVERITY[finding.severity]?.icon ?? 'i-lucide-info'" class="mt-0.5 size-4 shrink-0" :class="SEVERITY[finding.severity]?.class ?? 'text-info'" />
            <span>{{ viewLabel(answer, finding.view ?? 0) }}: {{ message(strings, finding) }}</span>
          </li>
        </ul>
      </template>
    </UAccordion>
  </section>
</template>
