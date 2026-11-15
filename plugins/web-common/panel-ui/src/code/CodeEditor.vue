<script setup lang="ts">
// A text field in syntax colours in a Nuxt UI panel (RB-080-84, RB-080-65): a textarea whose own
// text turns transparent - caret, selection and scrolling stay its own - over a backdrop that
// shows the same text coloured by the one highlighter (src/code/highlight.ts), spans and text
// only. Nuxt UI has no code field; this one is built from a plain textarea styled like UTextarea.
// `lang` is a language or a function of the text (a body by its headers' content type). Off
// (`ui.syntaxHighlight`), without a language or above the highlighter's limit the text shows as
// typed, and `note` says why above a large one. `data-highlight` says which: on, off, plain,
// large. A panel that uses it names this folder in its stylesheet (`@source`), as for CodeBlock.
// Both lie in the same box - no border, the same padding, font and wrapping (the ring is drawn
// inside) - and the backdrop keeps the textarea's scroll bar free and follows its scrolling, also
// when the colours come on after the field was scrolled (review of RB-080-84).
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { highlighter, markedTokens } from './highlight';
import { usePanel } from '../mount';

const props = defineProps<{
  lang: HighlightLang | null | ((text: string) => HighlightLang | null);
  note?: string;
  rows?: number;
  disabled?: boolean;
  id?: string;
  label?: string;
}>();
const model = defineModel<string>({ default: '' });
const { settings } = usePanel();
const back = ref<HTMLElement | null>(null);
const area = ref<HTMLTextAreaElement | null>(null);

watch(settings, (now) => void highlighter().configure(now), { immediate: true, flush: 'sync' });

const language = computed(() => (typeof props.lang === 'function' ? props.lang(model.value) : props.lang));
const outcome = computed<HighlightOutcome>(() => {
  void settings.value;
  const hl = highlighter();
  if (!hl.enabled) return 'off';
  if (!language.value) return 'plain';
  return hl.tooLarge(model.value) ? 'large' : 'on';
});
const tokens = computed(() => (outcome.value === 'on' ? markedTokens(model.value, language.value, []) : []));

/** The backdrop as the textarea is now: its scroll bar's width kept free, its scroll position. */
function sync(): void {
  const field = area.value;
  const backdrop = back.value;
  if (!field || !backdrop) return;
  const bar = Math.max(0, field.offsetWidth - field.clientWidth);
  backdrop.style.paddingRight = bar > 0 ? `calc(0.625rem + ${bar}px)` : '';
  backdrop.scrollTop = field.scrollTop;
  backdrop.scrollLeft = field.scrollLeft;
}

watch([outcome, model], () => void nextTick(sync));
let resized: ResizeObserver | undefined;
onMounted(() => {
  sync();
  if (typeof ResizeObserver !== 'undefined' && area.value) {
    resized = new ResizeObserver(() => sync());
    resized.observe(area.value);
  }
});
onBeforeUnmount(() => resized?.disconnect());
</script>

<template>
  <div class="relative" :data-highlight="outcome">
    <p v-if="outcome === 'large' && note" data-note class="mb-1 text-xs text-muted italic">{{ note }}</p>
    <div class="relative">
      <pre
        v-if="outcome === 'on'"
        ref="back"
        aria-hidden="true"
        class="pointer-events-none absolute inset-0 m-0 overflow-hidden rounded-md border-0 px-2.5 py-1.5 font-mono text-xs leading-5 whitespace-pre-wrap break-words"
      ><span v-for="(piece, index) in tokens" :key="index" :class="piece.kind === 'plain' ? undefined : 'hl-' + piece.kind">{{ piece.text }}</span>{{ '\n' }}</pre>
      <textarea
        :id="id"
        ref="area"
        v-model="model"
        :rows="rows ?? 4"
        :disabled="disabled"
        :aria-label="label"
        spellcheck="false"
        class="relative block w-full resize-y rounded-md border-0 bg-transparent px-2.5 py-1.5 font-mono text-xs leading-5 whitespace-pre-wrap break-words ring ring-inset ring-accented outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:cursor-not-allowed disabled:opacity-75"
        :class="outcome === 'on' ? 'text-transparent caret-(--ui-text-highlighted) selection:bg-primary/25' : 'text-highlighted'"
        @scroll="sync"
        @input="sync"
      />
    </div>
  </div>
</template>
