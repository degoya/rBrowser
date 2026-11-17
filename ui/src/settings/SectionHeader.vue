<script setup lang="ts">
// The eyebrow, heading and description every part of the settings opens with (RB-080-13, after
// rDownloader's `SectionHeader`): one owner for the block, so headings cannot drift apart. Three
// levels, a hierarchy rather than a size picker - the page (`h1`, the category or the search),
// a card (`h2`, a section), a part inside a card (`h3`). The description width is fixed per
// level: line length is a readability rule, not a per-site choice. Anything beside the heading -
// a button, a badge - stays in the caller's own row; this is the text block only. The eyebrow is
// `text-toned`, not the accent colour: at 11 px on a soft card the teal accent falls below 4.5:1.
const props = withDefaults(
  defineProps<{
    /** The small monospaced label above the heading. */
    eyebrow: string;
    title: string;
    /** One sentence under the heading, when the heading does not say enough on its own. */
    description?: string;
    level?: 'page' | 'card' | 'sub';
  }>(),
  { description: undefined, level: 'card' },
);

const TAGS = { page: 'h1', card: 'h2', sub: 'h3' } as const;
const HEADINGS = { page: 'text-lg', card: 'text-base', sub: 'text-sm' } as const;
const DESCRIPTIONS = {
  page: 'mt-1 max-w-3xl text-sm',
  card: 'mt-1 max-w-prose text-sm',
  sub: 'mt-0.5 max-w-prose text-xs',
} as const;
</script>

<template>
  <div class="min-w-0" data-testid="section-header" :data-level="props.level">
    <p class="font-mono text-[0.6875rem] font-semibold tracking-[0.14em] text-toned uppercase">
      {{ props.eyebrow }}
    </p>
    <component :is="TAGS[props.level]" :class="['font-semibold text-highlighted', HEADINGS[props.level]]">
      {{ props.title }}
    </component>
    <p v-if="props.description || $slots.description" :class="['text-muted', DESCRIPTIONS[props.level]]">
      <slot name="description">{{ props.description }}</slot>
    </p>
  </div>
</template>
