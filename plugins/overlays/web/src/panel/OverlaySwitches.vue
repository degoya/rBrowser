<script setup lang="ts">
// One group's overlays as switches (RB-080-86). A switch acts at once on every view; Shift held
// while it is clicked acts on the active view only, so the click - not the change - is read for
// Shift, before the switch reports its new value.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Toggle, toggleKey } from './model';

defineProps<{ toggles: readonly Toggle[]; shown: (toggle: Toggle) => boolean }>();
const emit = defineEmits<{ set: [toggle: Toggle, enabled: boolean, shift: boolean] }>();
const t = usePanel().t;

let shift = false;
function changed(toggle: Toggle, enabled: boolean): void {
  emit('set', toggle, enabled, shift);
  shift = false;
}
</script>

<template>
  <ul class="grid gap-x-4 gap-y-2 sm:grid-cols-2">
    <li v-for="toggle in toggles" :key="toggle" :data-toggle="toggle" @click.capture="shift = $event.shiftKey">
      <USwitch
        :id="'toggle-' + toggle"
        size="sm"
        :model-value="shown(toggle)"
        :label="t('panel.toggle.' + toggleKey(toggle))"
        @update:model-value="(enabled: boolean) => changed(toggle, enabled)"
      />
    </li>
  </ul>
</template>
