<script setup lang="ts">
// A settings mention as a link (RB-094-21, docs/design.md): beside a text that names a settings
// path ("Settings > Checks > Fonts > Largest preview file"), it opens Settings at `settingKey` -
// a key, a section or a category, the core's too - on a click (`panel.openSettings`, the host's
// `open_settings`). `label` is the plugin's own text ("Open the setting"); the key is its tooltip.
import { usePanel } from '../mount';

const props = defineProps<{ settingKey: string; label: string }>();
const panel = usePanel();
</script>

<template>
  <UButton
    size="sm"
    color="neutral"
    variant="link"
    class="px-0"
    icon="i-lucide-settings"
    :label="props.label"
    :title="props.settingKey"
    :data-testid="`setting-link-${props.settingKey}`"
    @click="panel.openSettings(props.settingKey)"
  />
</template>
