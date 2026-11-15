<script setup lang="ts">
// The question `ui.openLinksIn: ask` puts on every link (RB-080-92): the address's host, then
// "In your browser", "In the active view" or Cancel (Escape and a click outside cancel too).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UModal from '@nuxt/ui/components/Modal.vue';
import { hostOf } from './open';
import type { LinkOpener } from './opener';

const props = defineProps<{ opener: LinkOpener }>();
const { t } = useI18n();

const open = computed({
  get: () => props.opener.asking.value !== null,
  set: (value: boolean) => {
    if (!value) {
      props.opener.answer(null);
    }
  },
});
const host = computed(() => hostOf(props.opener.asking.value ?? ''));
</script>

<template>
  <UModal
    v-model:open="open"
    :title="t('links.choose.title')"
    :description="t('links.choose.description', { host })"
    data-testid="link-chooser"
  >
    <template #footer>
      <div class="flex flex-wrap justify-end gap-2">
        <UButton variant="ghost" data-testid="link-cancel" @click="opener.answer(null)">
          {{ t('links.choose.cancel') }}
        </UButton>
        <UButton variant="outline" data-testid="link-active-view" @click="opener.answer('activeView')">
          {{ t('links.choose.active_view') }}
        </UButton>
        <UButton data-testid="link-system" @click="opener.answer('system')">
          {{ t('links.choose.system') }}
        </UButton>
      </div>
    </template>
  </UModal>
</template>
