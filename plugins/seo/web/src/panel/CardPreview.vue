<script setup lang="ts">
// One link preview as a platform lays it out (RB-080-11, on Nuxt UI since RB-080-85): the image
// above the text cropped to 1.91:1 (`large`), a square thumbnail beside it (`small`) or the text
// alone (`text`), with the texts exactly as the component cut them - the full text and its tag as
// the tooltip, a note when it was cut. The only image source is a checked `data:` URI; the look
// follows the layout, not the platform's branding.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Card, type CardText, imageNote, imageOf, imageSize, type Layout, type Preview, safeDataUri } from './model';

const props = defineProps<{ preview: Preview; card: Card; layout: Layout }>();
const panel = usePanel();
const t = panel.t;

const image = computed(() => imageOf(props.preview, props.card));
const uri = computed(() => safeDataUri(image.value));
const size = computed(() => imageSize(t, image.value));
const note = computed(() => imageNote(t, image.value, (code) => panel.error(code)) || size.value);

const tooltip = (value: CardText): string => `${value.full} (${t('panel.source', { source: value.source })})`;
const cutNote = (value: CardText): string => t('panel.truncated', { count: [...value.full].length });
</script>

<template>
  <div
    data-preview
    :data-layout="layout"
    class="overflow-hidden rounded-md border border-default bg-default"
    :class="layout === 'small' ? 'flex' : ''"
  >
    <div
      v-if="layout !== 'text'"
      data-image
      class="flex items-center justify-center bg-elevated text-muted"
      :class="layout === 'large' ? 'aspect-[1.91/1] w-full' : 'size-20 shrink-0'"
    >
      <img v-if="uri" :src="uri" alt="" class="block size-full object-cover" />
      <span v-else data-note class="p-1.5 text-center text-xs">{{ note }}</span>
    </div>
    <div class="min-w-0 flex-1 px-2 py-1.5 text-sm">
      <p data-domain class="text-[0.6875rem] uppercase text-muted">{{ preview.domain }}</p>
      <p v-if="card.title" data-title class="font-semibold break-words text-highlighted" :title="tooltip(card.title)">{{ card.title.text }}</p>
      <p v-else data-title data-missing class="italic text-muted">{{ t('panel.no_title') }}</p>
      <p v-if="card.title?.truncated" data-cut class="text-xs italic text-muted">{{ cutNote(card.title) }}</p>
      <template v-if="card.description">
        <p data-description class="break-words text-muted" :title="tooltip(card.description)">{{ card.description.text }}</p>
        <p v-if="card.description.truncated" data-cut class="text-xs italic text-muted">{{ cutNote(card.description) }}</p>
      </template>
      <p v-if="size && layout !== 'text'" data-size class="text-xs text-muted">{{ size }}</p>
    </div>
  </div>
</template>
