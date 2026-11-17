<script setup lang="ts">
// The options of the highlight and force-state overlays (RB-080-86, docs/design.md rule 9: the
// options sit with the tool): the CSS selector - free text, a selector is the person's own -
// with Find and the steps through the matches in every view, and the state force-state forces,
// a choice of the known pseudo-classes. Enter in the field finds, the arrow keys step.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { FORCED_STATES } from './model';

const selector = defineModel<string>('selector', { required: true });
const forced = defineModel<string>('forced', { required: true });
const emit = defineEmits<{ find: []; step: [by: 1 | -1] }>();
const t = usePanel().t;

const STATES = FORCED_STATES.map((state) => ({ label: `:${state}`, value: state }));

function onKey(event: KeyboardEvent): void {
  if (event.key === 'Enter') {
    event.preventDefault();
    emit('find');
  } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault();
    emit('step', event.key === 'ArrowDown' ? 1 : -1);
  }
}
</script>

<template>
  <div class="flex flex-col gap-3">
    <UFormField size="sm" :label="t('panel.selector')" :help="t('panel.selector_hint')">
      <UFieldGroup size="sm" class="w-full">
        <UInput
          id="selector"
          v-model="selector"
          class="min-w-0 flex-1"
          autocomplete="off"
          :spellcheck="false"
          :maxlength="512"
          @keydown="onKey"
        />
        <UTooltip :text="t('panel.previous')">
          <UButton id="previous" color="neutral" variant="outline" icon="i-lucide-chevron-up" :aria-label="t('panel.previous')" @click="emit('step', -1)" />
        </UTooltip>
        <UTooltip :text="t('panel.next')">
          <UButton id="next" color="neutral" variant="outline" icon="i-lucide-chevron-down" :aria-label="t('panel.next')" @click="emit('step', 1)" />
        </UTooltip>
        <UButton id="find" color="primary" variant="soft" icon="i-lucide-search" :label="t('panel.find')" @click="emit('find')" />
      </UFieldGroup>
    </UFormField>
    <UFormField size="sm" :label="t('panel.state')">
      <USelect id="force-state" v-model="forced" :items="STATES" class="w-48 max-w-full" />
    </UFormField>
  </div>
</template>
