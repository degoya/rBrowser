<script setup lang="ts">
// A settings mention as a link (RB-094-21, docs/design.md): a text that names a settings path
// ("Settings > Automation > MCP") shows this button beside it, which opens Settings at the key
// the text means. The page sends the message (`openSettings`); its key is one of
// crates/rbrowser/data/settings-links.json, which the settings lint holds to the texts.
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { ICONS } from '../icons';
import type { ShellToCore } from '../ipc/protocol';

const props = defineProps<{ settingKey: string }>();
const emit = defineEmits<{ open: [message: ShellToCore] }>();
const { t } = useI18n();
</script>

<template>
  <UButton
    size="sm"
    color="neutral"
    variant="link"
    class="px-0"
    :icon="ICONS.settings"
    :label="t('shell.setting_link')"
    :aria-label="t('shell.status.open_setting_key', { key: props.settingKey })"
    :title="props.settingKey"
    :data-testid="`setting-link-${props.settingKey}`"
    @click="emit('open', { type: 'openSettings', key: props.settingKey })"
  />
</template>
