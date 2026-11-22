<script setup lang="ts">
// A form in a plugin panel (RB-096-13): a group of fields whose Enter answers, as a form's does.
// A panel runs in `<iframe sandbox="allow-scripts">` without `allow-forms` - the sandbox is a
// security boundary and stays - and Chromium checks it before it fires `submit`: a native
// `<form>`, even with `@submit.prevent`, never runs its handler there and only logs "Blocked form
// submission". So this is no form element: Enter in a text field emits `submit` (not in a text
// area, a checkbox, a button or a pick list, nor while a word is composed or while `disabled`),
// and the action's button is an ordinary `type="button"` with its own click. The attributes
// (`id`, `role`, `aria-label`, a `keydown.escape`) go to the group.
const props = defineProps<{ disabled?: boolean }>();
const emit = defineEmits<{ submit: [] }>();

/** The input types whose Enter is the form's: the text-like ones. */
const NOT_TEXT = new Set(['button', 'checkbox', 'radio', 'submit', 'reset', 'file', 'image', 'range', 'color']);

function enter(event: KeyboardEvent): void {
  const field = event.target;
  if (event.isComposing || props.disabled || !(field instanceof HTMLInputElement) || NOT_TEXT.has(field.type)) return;
  event.preventDefault();
  emit('submit');
}
</script>

<template>
  <div role="form" @keydown.enter="enter">
    <slot />
  </div>
</template>
