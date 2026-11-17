<script setup lang="ts">
// A pulldown of the toolbar (RB-090-45, `pulldown.ts`): a Nuxt UI button with the value in effect
// and a chevron, whose rows the core opens as a native menu below it (`nativeMenu.ts`), in
// rBrowser's theme like the theme menu and the canvas zoom's presets - never Chromium's
// `<select>` popup. A click, Enter, Space, an arrow key or F4 opens it, Esc closes it, the focus
// comes back to the button; every choice is told (`choose`), the one in effect too, so it can be
// chosen again (RB-080-16). Its group shrinks it to its icon in a narrow window; out of the row,
// its rows stand under its name in the overflow menu (`provideMenuSource`).
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { NativeMenuRow } from '../ipc/protocol';
import { useShell } from '../state/shell';
import { isOpeningKey, provideMenuSource, useNativeMenu, type PulldownKind } from './nativeMenu';
import { PULLDOWN_CHEVRON, useGroupLook } from './pulldown';

const props = withDefaults(
  defineProps<{
    kind: PulldownKind;
    /** What the control is ("Environment"): with the value its name, its tooltip and its heading. */
    label: string;
    /** The value in effect, or why there is none; without one the button shows `label`. */
    value?: string;
    /** The rows as the menu shows them now. */
    rows: () => NativeMenuRow[];
    disabled?: boolean;
  }>(),
  { value: undefined, disabled: false },
);
const emit = defineEmits<{ choose: [id: string] }>();

const shell = useShell();
const { t } = useI18n();
const look = useGroupLook();
const menu = useNativeMenu(props.kind, { send: shell.send });

const anchor = ref<HTMLElement | null>(null);
const name = computed(() =>
  props.value === undefined ? props.label : t('shell.toolbar.pulldown.name', { label: props.label, value: props.value }),
);
/** In a narrow window only the group's icon; without one the value stays. */
const compact = computed(() => look.compact.value && look.icon.value !== undefined);

function choose(id: string): void {
  emit('choose', id);
}

function open(): void {
  if (anchor.value !== null && !props.disabled) {
    void menu.open(anchor.value, props.rows(), choose);
  }
}

function onKeyDown(event: KeyboardEvent): void {
  if (isOpeningKey(event)) {
    event.preventDefault();
    open();
  }
}

/** The rows for the overflow menu, while the button is there. */
let stopSource: (() => void) | null = null;
watch(
  anchor,
  (element) => {
    stopSource?.();
    const button = element?.querySelector('button');
    stopSource = button ? provideMenuSource(button, { rows: () => props.rows(), choose }) : null;
  },
  { flush: 'post' },
);
onBeforeUnmount(() => stopSource?.());
</script>

<template>
  <span ref="anchor" class="inline-flex min-w-0 shrink-0 items-center">
    <UButton
      :label="compact ? undefined : (value ?? label)"
      :icon="compact ? look.icon.value : undefined"
      :trailing-icon="compact ? undefined : PULLDOWN_CHEVRON"
      color="neutral"
      variant="outline"
      size="sm"
      :square="compact"
      :class="compact ? 'size-8 justify-center' : 'max-w-40'"
      :disabled="disabled"
      aria-haspopup="menu"
      :aria-label="name"
      :title="name"
      data-pulldown
      @click="open"
      @keydown="onKeyDown"
    />
  </span>
</template>
