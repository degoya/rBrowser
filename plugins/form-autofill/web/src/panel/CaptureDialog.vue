<script setup lang="ts">
// "Take form as preset" (RB-090-70): the form read from a view, as an inline dialog at the top of
// the panel (a panel opens no modal, docs/design.md section 6) - the preset's name, a box per
// field that can be taken with the value it holds, and the fields that are not taken with the
// reason (a password, a secret or a file input never shows its value). Save stores the fields
// ticked through the tool `save` - the plugin's own file - and closes; a refusal stays here as
// one sentence. Esc or Cancel closes it.
import { computed, nextTick, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Capture, captureReason, chosenAtFirst, excludedOf, MAX_NAME, nameState, NO_PROJECT, saveArgs, takeableOf } from './capture';

const props = defineProps<{ capture: Capture | null }>();
const open = defineModel<boolean>('open', { required: true });
const emit = defineEmits<{ saved: [{ name: string; fields: number }] }>();
const panel = usePanel();
const t = panel.t;

const name = ref('');
const chosen = ref<string[]>([]);
const saving = ref(false);
const refusal = ref<string | null>(null);
const nameInput = ref<{ inputRef?: HTMLInputElement } | null>(null);

// A new capture starts from its own name and the fields that hold a value, the name focused.
watch(
  () => props.capture,
  (capture) => {
    name.value = capture?.name ?? '';
    chosen.value = capture ? chosenAtFirst(capture) : [];
    refusal.value = null;
  },
  { immediate: true },
);
watch(open, async (shown) => {
  if (!shown) return;
  await nextTick();
  nameInput.value?.inputRef?.focus();
});

const takeable = computed(() => (props.capture ? takeableOf(props.capture) : []));
const excluded = computed(() => (props.capture ? excludedOf(props.capture) : []));
const state = computed(() => (props.capture ? nameState(name.value, props.capture) : 'empty'));
const source = computed(() => {
  const capture = props.capture;
  if (!capture) return '';
  const form = capture.form?.name || capture.form?.selector || t('panel.capture.wholePage');
  return t('panel.capture.source', { view: capture.label, form });
});
// No project, or a presets file that does not read whole (said in words, Save off).
const noProject = computed(() => Boolean(props.capture && !props.capture.available && (props.capture.storeError?.code ?? NO_PROJECT) === NO_PROJECT));
const storeFailed = computed(() => {
  const failed = props.capture?.available === false ? props.capture.storeError : null;
  if (!failed?.code || failed.code === NO_PROJECT) return null;
  return t('panel.capture.storeFailed', { text: panel.error(Object.assign(new Error(failed.code), { params: failed.params })) });
});
const canSave = computed(
  () => Boolean(props.capture?.available) && !saving.value && chosen.value.length > 0 && (state.value === 'new' || state.value === 'replaces'),
);

function toggle(locator: string): void {
  const rest = chosen.value.filter((entry) => entry !== locator);
  chosen.value = rest.length === chosen.value.length ? [...rest, locator] : rest;
}

async function save(): Promise<void> {
  const capture = props.capture;
  if (!capture || !canSave.value) return;
  saving.value = true;
  refusal.value = null;
  try {
    const args = saveArgs(name.value, capture, chosen.value);
    const answer = (await panel.call({ tool: 'save' }, args)) as { saved?: string; fields?: number };
    emit('saved', { name: answer.saved ?? args.name, fields: answer.fields ?? Object.keys(args.values).length });
    open.value = false;
  } catch (error) {
    refusal.value = t('panel.capture.saveFailed', { text: panel.error(error) });
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <section v-if="open && capture" data-capture aria-labelledby="capture-title" @keydown.esc.stop="open = false">
    <UCard :ui="{ header: 'px-3 py-2 sm:px-3', body: 'px-3 py-3 sm:px-3', footer: 'px-3 py-2 sm:px-3' }">
      <template #header>
        <h2 id="capture-title" class="text-sm font-semibold text-highlighted">{{ t('panel.capture.title') }}</h2>
        <p id="capture-source" class="truncate text-xs text-muted" :title="capture.url">{{ source }}</p>
      </template>

      <div class="flex flex-col gap-3">
        <UFormField :label="t('panel.capture.name')" :help="t('panel.capture.nameHint')" name="preset-name">
          <UInput id="preset-name" ref="nameInput" v-model="name" :maxlength="MAX_NAME" class="w-full" @keydown.enter.prevent="save" />
        </UFormField>
        <UAlert v-if="state === 'replaces'" id="name-replaces" color="warning" variant="subtle" icon="i-lucide-triangle-alert" :title="t('panel.capture.replaces', { name: name.trim() })" />
        <UAlert v-else-if="state === 'inSettings'" id="name-in-settings" color="error" variant="subtle" icon="i-lucide-circle-x" :title="t('panel.capture.inSettings', { name: name.trim() })" />
        <UAlert v-if="noProject" id="no-project" color="warning" variant="subtle" icon="i-lucide-folder-x" :title="t('panel.capture.noProject')" />
        <UAlert v-if="storeFailed" id="store-failed" color="error" variant="subtle" icon="i-lucide-circle-x" :title="storeFailed" />
        <UAlert v-if="refusal" id="save-refused" color="error" variant="subtle" icon="i-lucide-circle-x" :title="refusal" />

        <div class="flex min-w-0 flex-col gap-1" role="group" aria-labelledby="capture-fields">
          <p id="capture-fields" class="text-sm font-medium text-highlighted">
            {{ t('panel.capture.fields') }}
            <span class="font-normal text-muted">{{ t('panel.capture.chosen', { count: chosen.length }) }}</span>
          </p>
          <p v-if="takeable.length === 0" id="no-fields" class="text-sm text-muted">{{ t('panel.capture.noFields') }}</p>
          <ul v-else data-capture-fields class="flex flex-col">
            <li v-for="(field, at) in takeable" :key="field.locator" class="min-w-0">
              <!-- A UButton as the field's checkbox (role, aria-checked, the box as its icon):
                   UCheckbox would carry the panel's script past panel-ui's 640 KiB budget
                   (measured 666 KiB with it), RB-090-70. -->
              <UButton
                :id="`take-${at}`"
                role="checkbox"
                :aria-checked="chosen.includes(field.locator) ? 'true' : 'false'"
                color="neutral"
                variant="ghost"
                size="sm"
                block
                :icon="chosen.includes(field.locator) ? 'i-lucide-square-check' : 'i-lucide-square'"
                :ui="{ base: 'justify-start gap-2 text-left', leadingIcon: chosen.includes(field.locator) ? 'text-primary' : 'text-muted' }"
                @click="toggle(field.locator)"
              >
                <span class="min-w-0 shrink-0 truncate font-medium" :title="field.label">{{ field.label }}</span>
                <span class="min-w-0 truncate text-muted" data-value :title="field.value">{{ field.value ? field.value : t('panel.emptyValue') }}</span>
              </UButton>
            </li>
          </ul>
          <p v-if="capture.truncated" id="capture-truncated" class="text-xs text-muted">{{ t('panel.capture.truncated') }}</p>
        </div>

        <div v-if="excluded.length > 0" data-excluded class="flex flex-col gap-1">
          <p class="text-sm font-medium text-highlighted">{{ t('panel.capture.excluded', { count: excluded.length }) }}</p>
          <ul class="flex flex-col gap-1">
            <li v-for="(field, at) in excluded" :key="`${field.locator}-${at}`" data-excluded-field class="flex min-w-0 items-center justify-between gap-2 px-2 text-sm">
              <span class="truncate" :title="field.label">{{ field.label }}</span>
              <UBadge color="neutral" variant="subtle" size="sm" data-reason :label="captureReason(t, field.excluded ?? '')" />
            </li>
          </ul>
        </div>
      </div>

      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton id="cancel-preset" color="neutral" variant="ghost" size="sm" :label="t('panel.capture.cancel')" @click="open = false" />
          <UButton id="save-preset" data-primary size="sm" icon="i-lucide-save" :label="t('panel.capture.save')" :loading="saving" :disabled="!canSave" @click="save" />
        </div>
      </template>
    </UCard>
  </section>
</template>
