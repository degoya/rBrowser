<script setup lang="ts">
// The large view of the sitemap gallery page (RB-090-86): one shot at full width in a modal,
// Previous and Next through the shots the sheet shows (`shotOrder`), the arrow keys doing the
// same and Escape closing it (the modal's own).
import { computed, onBeforeUnmount, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UModal from '@nuxt/ui/components/Modal.vue';
import type { SitemapGalleryPage } from '../../ipc/protocol';
import { labelOf, step, type ShotRef } from './gallery';
import { GALLERY_ICONS } from './icons';

const props = defineProps<{ run: SitemapGalleryPage; order: ShotRef[]; shot: ShotRef | null }>();
const emit = defineEmits<{ show: [shot: ShotRef | null] }>();
const { t } = useI18n();

const open = computed({
  get: () => props.shot !== null,
  set: (value: boolean) => {
    if (!value) emit('show', null);
  },
});
const page = computed(() => (props.shot === null ? null : props.run.pages[props.shot.page] ?? null));
const breakpoint = computed(() => props.run.breakpoints.find((bp) => bp.name === props.shot?.breakpoint) ?? null);
const image = computed(() => (props.shot === null ? undefined : page.value?.shots[props.shot.breakpoint]?.image));
const label = computed(() => (breakpoint.value === null ? '' : labelOf(breakpoint.value)));
const at = computed(() => props.order.findIndex((shot) => shot.page === props.shot?.page && shot.breakpoint === props.shot?.breakpoint));
const previous = computed(() => (props.shot === null ? null : step(props.order, props.shot, -1)));
const next = computed(() => (props.shot === null ? null : step(props.order, props.shot, 1)));

function go(target: ShotRef | null): void {
  if (target !== null) emit('show', target);
}

function onKey(event: KeyboardEvent): void {
  if (props.shot === null || event.altKey || event.ctrlKey || event.metaKey) return;
  if (event.key === 'ArrowLeft') {
    event.preventDefault();
    go(previous.value);
  } else if (event.key === 'ArrowRight') {
    event.preventDefault();
    go(next.value);
  }
}

onMounted(() => window.addEventListener('keydown', onKey));
onBeforeUnmount(() => window.removeEventListener('keydown', onKey));
</script>

<template>
  <UModal
    v-model:open="open"
    fullscreen
    :title="page?.url ?? ''"
    :description="t('sitemap.galleryPage.view.position', { breakpoint: label, at: at + 1, total: order.length })"
    data-testid="shot-viewer"
  >
    <template #body>
      <img v-if="image" :src="image" :alt="t('sitemap.galleryPage.shot.alt', { url: page?.url ?? '', breakpoint: label })" class="mx-auto h-auto max-w-full" data-testid="shot-large" />
    </template>
    <template #footer>
      <div class="flex w-full flex-wrap justify-between gap-2">
        <div class="flex gap-2">
          <UButton :icon="GALLERY_ICONS.galleryPrevious" variant="outline" :disabled="previous === null" data-testid="shot-previous" @click="go(previous)">
            {{ t('sitemap.galleryPage.view.previous') }}
          </UButton>
          <UButton :trailing-icon="GALLERY_ICONS.galleryNext" variant="outline" :disabled="next === null" data-testid="shot-next" @click="go(next)">
            {{ t('sitemap.galleryPage.view.next') }}
          </UButton>
        </div>
        <UButton variant="ghost" data-testid="shot-close" @click="emit('show', null)">{{ t('sitemap.galleryPage.view.close') }}</UButton>
      </div>
    </template>
  </UModal>
</template>
