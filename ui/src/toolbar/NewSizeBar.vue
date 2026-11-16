<script setup lang="ts">
// "Custom..." of the new-size picker (RB-080-41, RB-080-46): `view.add_size { custom: true }`
// reaches the shell as the `uiCommand` `view.add_size.custom`, and this bar covers the toolbar
// strip with a size field and an optional name. Enter in either field adds a breakpoint of that
// size to `config.local.json` and opens its view; a refused input says why at the bar; Escape or
// leaving the bar closes it. Mounted in `App.vue`, beside the free size input of RB-010-16.
//
// The bar hears the Enter key itself: a form with two text fields and no submit button is never
// submitted by the browser (HTML's implicit submission), which is why Enter did nothing.
// Closed by Escape or done, the focus goes back to its opener (`returnFocus.ts`, RB-090-23).
// The fields are `UInput`s (RB-080-88).
import { computed, nextTick, onScopeDispose, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UInput from '@nuxt/ui/components/Input.vue';
import { parseSize } from '../breakpoint_keys/size';
import { returnFocus } from '../focus/returnFocus';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { toMessage } from './actions';
import { CUSTOM_SIZE_COMMAND } from './addView';

/** A breakpoint name: 1 to 40 letters, digits, `-`, `_` or `.`, as the core checks it. */
const NAME = /^[A-Za-z0-9_.-]{1,40}$/;

/** Why the last Enter added nothing. */
type Reason = 'size' | 'name' | 'refused';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();

const open = ref(false);
const text = ref('');
const name = ref('');
const reason = ref<Reason | null>(null);
/** The core's own words for a refusal. */
const refusal = ref('');
const field = ref<{ inputRef?: HTMLInputElement | null } | null>(null);
const focus = returnFocus();
const form = ref<HTMLFormElement | null>(null);

const hint = computed(() => {
  switch (reason.value) {
    case 'size':
      return t('views.new_size.invalid');
    case 'name':
      return t('views.new_size.error.name');
    case 'refused':
      return t('views.new_size.error.refused', { message: refusal.value });
    default:
      return t('views.new_size.hint');
  }
});

// Typing again takes the reason away.
watch([text, name], () => (reason.value = null));

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'uiCommand' && message.id === CUSTOM_SIZE_COMMAND) {
    text.value = '';
    name.value = '';
    reason.value = null;
    focus.remember();
    open.value = true;
    void nextTick(() => field.value?.inputRef?.focus());
  }
});
onScopeDispose(stop);

function close(): void {
  open.value = false;
  focus.restore();
}

/** Closes without moving the focus: it left the bar for where the reader put it. */
function leave(): void {
  open.value = false;
  focus.forget();
}

// Leaving the bar (not just one of its fields) closes it.
function onFocusOut(event: FocusEvent): void {
  const next = event.relatedTarget;
  if (!(next instanceof Node) || !form.value?.contains(next)) {
    leave();
  }
}

function onEnter(event: KeyboardEvent): void {
  // An input method's Enter confirms its composition, not the bar.
  if (!event.isComposing) {
    void submit();
  }
}

async function submit(): Promise<void> {
  const size = parseSize(text.value);
  const trimmed = name.value.trim();
  if (size === null) {
    reason.value = 'size';
    return;
  }
  if (trimmed !== '' && !NAME.test(trimmed)) {
    reason.value = 'name';
    return;
  }
  const said = await shell.refused(
    toMessage({
      command: 'view.add_size',
      width: size.width,
      height: size.height,
      ...(size.dpr === undefined ? {} : { dpr: size.dpr }),
      ...(trimmed === '' ? {} : { name: trimmed }),
    }),
  );
  if (said === null) {
    close();
  } else {
    refusal.value = said;
    reason.value = 'refused';
  }
}
</script>

<template>
  <form
    v-if="open"
    ref="form"
    class="fixed inset-x-0 top-0 z-50 flex h-11 items-center gap-2 border-b border-default bg-elevated px-3 text-sm text-default"
    data-testid="new-size"
    @submit.prevent
    @focusout="onFocusOut"
    @keydown.enter.prevent="onEnter"
    @keydown.escape.prevent="close"
  >
    <label for="rb-new-size" class="shrink-0 text-muted">{{ t('views.new_size.label') }}</label>
    <UInput
      id="rb-new-size"
      ref="field"
      v-model="text"
      type="text"
      size="sm"
      class="w-40"
      :ui="{ base: 'font-mono tabular-nums' }"
      inputmode="decimal"
      autocomplete="off"
      spellcheck="false"
      :color="reason === 'size' ? 'error' : 'primary'"
      :highlight="reason === 'size'"
      :placeholder="t('views.new_size.placeholder')"
      :aria-invalid="reason === 'size' ? 'true' : undefined"
      aria-describedby="rb-new-size-hint"
      data-testid="new-size-field"
    />
    <label for="rb-new-size-name" class="shrink-0 text-muted">{{ t('views.new_size.name') }}</label>
    <UInput
      id="rb-new-size-name"
      v-model="name"
      type="text"
      size="sm"
      class="w-32"
      autocomplete="off"
      spellcheck="false"
      :color="reason === 'name' ? 'error' : 'primary'"
      :highlight="reason === 'name'"
      :placeholder="t('views.new_size.name_placeholder')"
      :aria-invalid="reason === 'name' ? 'true' : undefined"
      aria-describedby="rb-new-size-hint"
      data-testid="new-size-name"
    />
    <span
      id="rb-new-size-hint"
      class="truncate text-xs"
      :class="reason === null ? 'text-muted' : 'text-error'"
      :role="reason === null ? undefined : 'alert'"
      :title="hint"
      :data-reason="reason ?? undefined"
      data-testid="new-size-hint"
    >
      {{ hint }}
    </span>
  </form>
</template>
