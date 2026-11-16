<script setup lang="ts">
// Settings > About (RB-010-18, after rCommander's RC-010-19): two tabs, About (the build, the
// engines, the API versions, the folders, "Copy version info", the links, the author) and
// Licences (every bundled component with its licence). The facts come from the core's
// `getAbout`, the same function MCP's `app_info` answers from.
import { onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import TabStrip from '../settings/TabStrip.vue';
import type { IpcClient } from '../ipc/client';
import type { AboutInfo } from '../ipc/protocol';
import AboutTab from './AboutTab.vue';
import LicencesTab from './LicencesTab.vue';
import DiagnosticsSave from '../settings/DiagnosticsSave.vue';

const props = defineProps<{ client: IpcClient }>();
const { t } = useI18n();
const tab = ref<string | number>('about');
const info = ref<AboutInfo | null>(null);
const loadError = ref<string | null>(null);

const tabs = [
  { label: t('settings.about.tab.about'), value: 'about' },
  { label: t('settings.about.tab.licences'), value: 'licences' },
];

onMounted(async () => {
  try {
    const { type: _type, ...facts } = await props.client.expect({ type: 'getAbout' }, 'about');
    info.value = facts;
  } catch (error) {
    loadError.value = error instanceof Error ? error.message : String(error);
  }
});
</script>

<template>
  <section class="flex flex-col gap-4" data-testid="about-page">
    <TabStrip v-model="tab" :items="tabs" variant="link" data-testid="about-tabs" />
    <template v-if="tab === 'about'">
      <p v-if="loadError" class="text-sm text-error" role="alert">
        {{ t('settings.about.load_failed', { reason: loadError }) }}
      </p>
      <AboutTab v-else-if="info" :client="client" :info="info" />
      <!-- RB-080-73: the diagnostics ZIP for a bug report, where testers look for it. -->
      <DiagnosticsSave :client="client" />
    </template>
    <LicencesTab v-else />
  </section>
</template>
