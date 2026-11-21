<script setup lang="ts">
// A text of the panel that names a settings path (RB-096-11, RB-094-21): the text as it is, then
// "Open the setting", a link that opens the settings window at `settingKey` (the host's
// `open_settings`). Each use passes the key as a literal `setting-key` attribute, which the settings
// lint (scripts/lib/settings_targets.py) reads and resolves.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';

defineProps<{ text: string; params?: Record<string, string | number>; settingKey: string }>();
const panel = usePanel();
const t = panel.t;
</script>

<template>
  <span data-settings-text>
    {{ t(text, params ?? {}) }}
    <UButton
      variant="link"
      color="neutral"
      size="xs"
      icon="i-lucide-settings"
      class="px-0 align-baseline"
      :data-setting="settingKey"
      :title="settingKey"
      :label="t('panel.openSetting')"
      @click="panel.openSettings(settingKey)"
    />
  </span>
</template>
