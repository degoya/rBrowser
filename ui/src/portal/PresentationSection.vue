<script setup lang="ts">
// The presentation over the internet in the Portal panel (RB-096-05): off, the consent before
// the first start (the traffic runs through a Cloudflare Quick Tunnel, meant for tests and
// demonstrations; the download of cloudflared named when one is needed), the tunnel opening,
// and live: the viewers, the one-time link with Copy and its QR code (drawn from the core's
// module rows, never markup), a new link and the stop. Every button runs a command of the
// registry; only the clicked page writes the clipboard. Off, the hint opens Settings at
// `portal.presentation.enabled` (RB-096-09).
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import { ICONS } from '../icons';
import type { PortalPresentation, ShellToCore } from '../ipc/protocol';
import { clock, qrPath } from './portal';
import { linkSecondsLeft, newLinkMessage, startMessage, stopMessage } from './presentation';

const props = withDefaults(
  defineProps<{
    presentation: PortalPresentation;
    nowMs: number;
    /** The clipboard written; the browser's when absent. */
    clipboard?: Pick<Clipboard, 'writeText'>;
  }>(),
  { clipboard: undefined },
);
const emit = defineEmits<{ run: [message: ShellToCore] }>();
/** The setting the off hint opens Settings at (RB-096-09); only a person switches it on. */
const ENABLED_SETTING = 'portal.presentation.enabled';
const { t } = useI18n();
const copied = ref(false);

const link = computed(() => props.presentation.link ?? null);
const left = computed(() => linkSecondsLeft(props.presentation, props.nowMs));
const path = computed(() => (link.value ? qrPath(link.value.rows) : ''));
const download = computed(() => props.presentation.download ?? null);

watch(
  () => link.value?.url,
  () => {
    copied.value = false;
  },
);

function start(): void {
  emit(
    'run',
    startMessage({ consent: !props.presentation.consented, download: download.value !== null }),
  );
}

async function copy(): Promise<void> {
  if (link.value === null) return;
  try {
    await (props.clipboard ?? navigator.clipboard).writeText(link.value.url);
    copied.value = true;
  } catch {
    copied.value = false;
  }
}
</script>

<template>
  <section class="flex flex-col gap-3 border-t border-default pt-4" data-testid="presentation">
    <h2 class="font-medium">{{ t('portal.present.panel.title') }}</h2>
    <div v-if="!presentation.enabled" class="flex flex-col items-start gap-2">
      <p class="text-sm" data-testid="presentation-disabled">
        {{ t('portal.present.panel.disabled') }}
      </p>
      <UButton
        size="sm"
        color="neutral"
        variant="outline"
        :icon="ICONS.settings"
        data-testid="presentation-open-setting"
        @click="emit('run', { type: 'openSettings', key: ENABLED_SETTING })"
      >
        {{ t('portal.present.panel.open_setting') }}
      </UButton>
    </div>
    <template v-else-if="presentation.phase === 'off'">
      <UAlert
        v-if="!presentation.consented"
        color="warning"
        variant="subtle"
        :title="t('portal.present.panel.consent_title')"
        :description="t('portal.present.panel.consent')"
        data-testid="presentation-consent"
      />
      <p class="text-sm text-muted" data-testid="presentation-terms">{{ t('portal.present.panel.terms') }}</p>
      <p v-if="download" class="text-sm" data-testid="presentation-download">
        {{
          t('portal.present.panel.download', {
            version: download.version,
            size: download.sizeMb,
            license: download.license,
            from: download.from,
          })
        }}
      </p>
      <div>
        <UButton size="sm" color="primary" data-testid="presentation-start" @click="start">
          {{
            !presentation.consented
              ? t('portal.present.panel.agree_start')
              : download
                ? t('portal.present.panel.download_start')
                : t('portal.present.panel.start')
          }}
        </UButton>
      </div>
    </template>
    <p v-else-if="presentation.phase === 'downloading'" class="text-sm" data-testid="presentation-downloading">
      {{ t('portal.present.panel.downloading') }}
    </p>
    <template v-else>
      <p v-if="presentation.phase === 'starting'" class="text-sm" data-testid="presentation-starting">
        {{ t('portal.present.panel.starting') }}
      </p>
      <p v-else class="text-sm text-warning" data-testid="presentation-status">
        {{
          t('portal.present.panel.live', {
            address: presentation.address ?? '',
            count: presentation.viewers,
            max: presentation.maxViewers,
          })
        }}
      </p>
      <template v-if="presentation.phase === 'live'">
        <div v-if="link && left > 0" class="flex flex-col items-start gap-2">
          <div class="flex w-full max-w-xl items-center gap-2">
            <UInput
              :model-value="link.url"
              readonly
              size="sm"
              class="flex-1"
              :aria-label="t('portal.present.panel.link_label')"
              data-testid="presentation-link"
            />
            <UButton size="sm" variant="outline" data-testid="presentation-copy" @click="copy">
              {{ copied ? t('portal.present.panel.copied') : t('portal.present.panel.copy') }}
            </UButton>
          </div>
          <figure class="flex flex-col items-start gap-2" data-testid="presentation-qr">
            <svg
              :viewBox="`-4 -4 ${link.size + 8} ${link.size + 8}`"
              class="h-48 w-48 bg-white"
              shape-rendering="crispEdges"
              role="img"
              :aria-label="t('portal.present.panel.qr_label')"
            >
              <path :d="path" fill="#000" />
            </svg>
            <figcaption class="text-sm">
              {{ t('portal.present.panel.link_caption', { time: clock(left) }) }}
            </figcaption>
          </figure>
        </div>
        <p v-else class="text-sm" data-testid="presentation-no-link">{{ t('portal.present.panel.no_link') }}</p>
      </template>
      <div class="flex flex-wrap gap-2">
        <UButton
          v-if="presentation.phase === 'live'"
          size="sm"
          variant="outline"
          data-testid="presentation-new-link"
          @click="emit('run', newLinkMessage())"
        >
          {{ t('portal.present.panel.new_link') }}
        </UButton>
        <UButton size="sm" color="error" data-testid="presentation-stop" @click="emit('run', stopMessage())">
          {{ t('portal.present.panel.stop') }}
        </UButton>
      </div>
    </template>
  </section>
</template>
