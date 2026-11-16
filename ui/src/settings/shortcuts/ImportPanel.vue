<script setup lang="ts">
// Export and import of the shortcut file in the shortcut editor. Export fills the field. Import
// checks the pasted file first (RB-060-11, `importDiff.ts`) and shows what would change, which
// commands it names that do not exist and the bindings two commands would hold; only "Apply"
// sends it to the core, which checks it again and skips what it refuses.
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UKbd from '@nuxt/ui/components/Kbd.vue';
import UTextarea from '@nuxt/ui/components/Textarea.vue';
import { SETTINGS_ICONS } from '../icons';
import { diffImport, parseShortcutFile, type ImportDiff } from './importDiff';
import type { ShortcutsStore } from './store';

const props = defineProps<{ store: ShortcutsStore }>();
const { t } = useI18n();
const transfer = ref('');
const preview = ref<ImportDiff | null>(null);
const errorKey = ref<string | null>(null);

const titleOf = (id: string): string => {
  const command = props.store.commands.value.find((c) => c.id === id);
  return command === undefined ? id : t(command.titleKey);
};
const nothingChanges = computed(
  () => preview.value !== null && preview.value.changes.length === 0 && preview.value.unknown.length === 0,
);

async function exportBindings(): Promise<void> {
  transfer.value = (await props.store.exportJson()) ?? '';
  preview.value = null;
}

function check(): void {
  const parsed = parseShortcutFile(transfer.value);
  if (!parsed.ok) {
    errorKey.value = parsed.errorKey;
    preview.value = null;
    return;
  }
  errorKey.value = null;
  preview.value = diffImport(props.store.commands.value, parsed.bindings);
}

async function apply(): Promise<void> {
  if (await props.store.importJson(transfer.value)) {
    transfer.value = '';
    preview.value = null;
  }
}

async function copy(): Promise<void> {
  try {
    await navigator.clipboard.writeText(transfer.value);
  } catch {
    // The text stays selectable in the field.
  }
}
</script>

<template>
  <div class="flex flex-col gap-2 border-t border-default pt-4">
    <h3 class="text-sm font-semibold text-highlighted">{{ t('settings.shortcuts.transfer') }}</h3>
    <UTextarea
      v-model="transfer"
      :rows="6"
      class="w-full font-mono"
      :aria-label="t('settings.shortcuts.transfer')"
      :placeholder="t('settings.shortcuts.transfer_hint')"
      data-testid="transfer"
      @update:model-value="preview = null"
    />
    <div class="flex flex-wrap gap-2">
      <UButton :icon="SETTINGS_ICONS.export" color="neutral" variant="soft" size="sm" :label="t('settings.shortcuts.export')" data-testid="export" @click="exportBindings" />
      <UButton :icon="SETTINGS_ICONS.copy" color="neutral" variant="soft" size="sm" :label="t('settings.json.copy')" :disabled="transfer === ''" @click="copy" />
      <UButton :icon="SETTINGS_ICONS.import" color="neutral" variant="soft" size="sm" :label="t('settings.shortcuts.import')" :disabled="transfer.trim() === ''" data-testid="import" @click="check" />
    </div>
    <p v-if="errorKey" class="text-sm text-error" role="alert" data-testid="import-error">{{ t(errorKey) }}</p>
    <section
      v-if="preview"
      class="flex flex-col gap-2 rounded-md border border-default p-3 text-sm"
      :aria-label="t('settings.shortcuts.import_preview')"
      data-testid="import-preview"
    >
      <h4 class="font-semibold text-highlighted">{{ t('settings.shortcuts.import_preview') }}</h4>
      <p v-if="nothingChanges" class="text-muted">{{ t('settings.shortcuts.import_no_changes') }}</p>
      <ul v-if="preview.changes.length > 0" class="flex flex-col gap-1">
        <li v-for="change in preview.changes" :key="change.id" class="flex flex-wrap items-center gap-1">
          <span class="min-w-48">{{ titleOf(change.id) }}</span>
          <UKbd v-for="b in change.before" :key="`b-${b}`" :value="b" size="sm" />
          <span v-if="change.before.length === 0" class="text-xs text-muted">{{ t('settings.shortcuts.unbound') }}</span>
          <span class="text-xs text-muted">{{ t('settings.shortcuts.import_to') }}</span>
          <UKbd v-for="a in change.after" :key="`a-${a}`" :value="a" size="sm" />
          <span v-if="change.after.length === 0" class="text-xs text-muted">{{ t('settings.shortcuts.unbound') }}</span>
        </li>
      </ul>
      <p v-if="preview.unknown.length > 0" class="text-warning">
        {{ t('settings.shortcuts.import_unknown', { ids: preview.unknown.join(', ') }) }}
      </p>
      <div v-if="preview.conflicts.length > 0" role="alert" class="text-error" data-testid="import-conflicts">
        <p class="font-semibold">{{ t('settings.shortcuts.import_conflicts') }}</p>
        <p v-for="c in preview.conflicts" :key="`${c.binding}-${c.second}`">
          {{ t('settings.shortcuts.import_conflict', { binding: c.binding, first: titleOf(c.first), second: titleOf(c.second) }) }}
        </p>
      </div>
      <div class="flex flex-wrap gap-2">
        <UButton
          size="sm"
          :color="preview.conflicts.length > 0 ? 'warning' : 'primary'"
          :label="preview.conflicts.length > 0 ? t('settings.shortcuts.import_apply_conflicts') : t('settings.shortcuts.import_apply')"
          data-testid="import-apply"
          @click="apply"
        />
        <UButton size="sm" color="neutral" variant="ghost" :label="t('settings.shortcuts.import_cancel')" data-testid="import-cancel" @click="preview = null" />
      </div>
    </section>
  </div>
</template>
