<script setup lang="ts">
// The free size input of RB-010-16: `view.set_size` without arguments (`Leader V S`, the
// "Custom size" row of a cell's quick pick) reaches the shell as `uiCommand`, and this bar covers
// the toolbar strip (the shell view, never the page) with one field. Enter sets the active view
// to `B x H [@DPR]`, Escape or leaving the field closes it. The coordinator mounts it in `App.vue`.
// Closed by Escape, Cancel or done, the focus goes back to its opener (`returnFocus.ts`, RB-090-23).
// The field is a `UInput` (RB-080-88).
import { computed, nextTick, onScopeDispose, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UInput from '@nuxt/ui/components/Input.vue';
import { returnFocus } from '../focus/returnFocus';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { OPEN_COMMAND, parseSize } from './size';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();

const open = ref(false);
const text = ref('');
const invalid = ref(false);
const field = ref<{ inputRef?: HTMLInputElement | null } | null>(null);
const focus = returnFocus();
const view = computed(() => shell.state.value?.activeView ?? shell.state.value?.views[0]?.id ?? null);

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'uiCommand' && message.id === OPEN_COMMAND) {
    const current = shell.state.value?.views.find((v) => v.id === view.value);
    text.value = current === undefined ? '' : `${current.width}x${current.height}`;
    invalid.value = false;
    focus.remember();
    open.value = true;
    void nextTick(() => {
      field.value?.inputRef?.focus();
      field.value?.inputRef?.select();
    });
  }
});
onScopeDispose(stop);

function close(): void {
  open.value = false;
  focus.restore();
}

/** Closes without moving the focus: it left the field for where the reader put it. */
function leave(): void {
  open.value = false;
  focus.forget();
}

async function submit(): Promise<void> {
  const size = parseSize(text.value);
  if (size === null || view.value === null) {
    invalid.value = true;
    return;
  }
  const sent = await shell.send({ type: 'setBreakpoint', view: view.value, ...size });
  if (sent) {
    close();
  } else {
    invalid.value = true;
  }
}
</script>

<template>
  <form
    v-if="open"
    class="fixed inset-x-0 top-0 z-50 flex h-11 items-center gap-2 border-b border-default bg-elevated px-3 text-sm text-default"
    data-testid="size-input"
    @submit.prevent="submit"
  >
    <label for="rb-size-input" class="shrink-0 text-muted">{{ t('sizes.input.label', { view: view ?? '' }) }}</label>
    <UInput
      id="rb-size-input"
      ref="field"
      v-model="text"
      type="text"
      size="sm"
      class="w-48"
      :ui="{ base: 'font-mono tabular-nums' }"
      inputmode="decimal"
      autocomplete="off"
      spellcheck="false"
      :color="invalid ? 'error' : 'primary'"
      :highlight="invalid"
      :placeholder="t('sizes.input.placeholder')"
      :aria-invalid="invalid ? 'true' : undefined"
      aria-describedby="rb-size-input-hint"
      data-testid="size-input-field"
      @keydown.escape.prevent="close"
      @blur="leave"
    />
    <span id="rb-size-input-hint" class="truncate text-xs" :class="invalid ? 'text-error' : 'text-muted'">
      {{ invalid ? t('sizes.input.invalid') : t('sizes.input.hint') }}
    </span>
  </form>
</template>
