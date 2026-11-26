<script setup lang="ts">
// Google's terms before the adb download (RB-099-13): `tools.adb.install` without the acceptance
// (the devices menu's "Download adb", the error toast's download button, `Leader X S`, the
// command palette) reaches the shell as `uiCommand` with what would be downloaded and the page of
// Google's terms (`adbTerms.ts`), and this bar covers the toolbar strip with it: the text, "Read
// the terms" (an `ExternalLink`, opened where `ui.openLinksIn` says), "Accept and download" - the
// only way the download starts - and Cancel. Escape and Cancel send nothing. The bar takes the
// focus itself, not a button, so a stray Enter accepts nothing; closed, the focus goes back to
// its opener (`returnFocus.ts`). Mounted in `App.vue`.
import { computed, nextTick, onScopeDispose, ref, useId } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import ExternalLink from '../links/ExternalLink.vue';
import { returnFocus } from '../focus/returnFocus';
import { ADB_INSTALL } from '../menu/devicesMenu';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { acceptance, adbTermsOf, type AdbTerms } from './adbTerms';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();
const id = useId();

const terms = ref<AdbTerms | null>(null);
const bar = ref<HTMLElement | null>(null);
const focus = returnFocus();
const text = computed(() => {
  const open = terms.value;
  return open === null
    ? ''
    : t('engines.android.terms.textLicense', { version: open.version, size: open.size, from: open.from, license: open.license });
});

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type !== 'uiCommand' || message.id !== ADB_INSTALL) {
    return;
  }
  const question = adbTermsOf(message.args);
  if (question === null) {
    return;
  }
  if (terms.value === null) {
    focus.remember();
  }
  terms.value = question;
  void nextTick(() => bar.value?.focus());
});
onScopeDispose(stop);

function close(): void {
  terms.value = null;
  focus.restore();
}

function accept(): void {
  // A refusal of the core reaches the status bar; the terms were accepted either way.
  void shell.send(acceptance());
  close();
}
</script>

<template>
  <div
    v-if="terms !== null"
    ref="bar"
    role="alertdialog"
    :aria-label="t('engines.android.terms.label')"
    :aria-describedby="`${id}-text`"
    class="fixed inset-x-0 top-0 z-50 flex h-11 items-center gap-2 overflow-x-auto border-b border-default bg-elevated px-3 text-sm text-default"
    data-testid="adb-terms"
    tabindex="-1"
    @keydown.escape.prevent="close"
  >
    <span :id="`${id}-text`" class="min-w-0 flex-1 truncate" :title="text" data-testid="adb-terms-text">{{ text }}</span>
    <ExternalLink :href="terms.terms" class="shrink-0 text-primary underline" data-testid="adb-terms-link">
      {{ t('engines.android.terms.link') }}
    </ExternalLink>
    <UButton size="sm" class="shrink-0" data-testid="adb-terms-accept" @click="accept">
      {{ t('engines.android.terms.accept') }}
    </UButton>
    <UButton size="sm" color="neutral" variant="ghost" class="shrink-0" data-testid="adb-terms-cancel" @click="close">
      {{ t('engines.android.terms.cancel') }}
    </UButton>
  </div>
</template>
