<script setup lang="ts">
// The entry editor (RB-070-01, on Nuxt UI since RB-080-84): a card under the grid for one cell -
// the whole value loaded from the view, save it there or in every view, delete it in one or (after
// an inline confirmation with the count) in every view, pin it - or empty to add an entry in one
// view or all. Cookies show their attributes (path, expiry, SameSite as a choice, HttpOnly,
// Secure). Escape closes it; the outcome stands in its own status line. The confirmation takes the
// focus on Cancel and hands it back to "Delete in all views" (RB-090-23).
import { computed, nextTick, onMounted, reactive, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import ValueField from './ValueField.vue';
import { type StoragePin, type StorageRow, type StorageView, entryArgs, expiresText, pinOf } from './model';
import { type EntryForm, writeEntry } from './write';

const props = defineProps<{ kind: string; row?: StorageRow; view?: StorageView; views: StorageView[]; pins: StoragePin[] }>();
const emit = defineEmits<{ close: []; written: []; pins: [pins: StoragePin[]] }>();
const panel = usePanel();
const t = panel.t;

const adding = computed(() => !(props.row && props.view));
const source = computed(() => (props.row ? (props.row.cells[String(props.view?.id)] ?? Object.values(props.row.cells).find(Boolean) ?? null) : null));
const form = reactive({
  name: props.row?.name ?? '',
  scope: props.row?.scope ?? '',
  path: props.row?.path ?? '/',
  expires: typeof source.value?.expires === 'number' ? expiresText(source.value.expires, '') : '',
  sameSite: source.value?.sameSite ?? '',
  httpOnly: !!source.value?.httpOnly,
  secure: !!source.value?.secure,
});
const target = ref('all');
const sessionOnly = ref(false);
const status = ref('');
const confirming = ref(false);
const value = ref<InstanceType<typeof ValueField> | null>(null);
const nameField = ref<{ inputRef?: HTMLInputElement } | null>(null);

const pinned = computed(() => (props.row ? pinOf(props.pins, props.kind, props.row) : undefined));
const targets = computed(() => [{ label: t('panel.allViews'), value: 'all' }, ...props.views.filter((one) => one.status === 'ok').map((one) => ({ label: one.label, value: String(one.id) }))]);
const SAME_SITE = ['', 'Strict', 'Lax', 'None'];
const sameSites = computed(() => SAME_SITE.map((one) => ({ label: one || t('panel.sameSiteUnset'), value: one || 'unset' })));
const sameSite = computed({ get: () => form.sameSite || 'unset', set: (next: string) => (form.sameSite = next === 'unset' ? '' : next) });
const okViews = computed(() => props.views.filter((one) => one.status === 'ok').length);

const entry = (): EntryForm => ({ ...form, domain: form.scope, value: value.value?.get() ?? '' });

async function write(op: 'set' | 'delete', where: string | number): Promise<void> {
  confirming.value = false;
  status.value = await writeEntry(panel, props.kind, op, where, entry());
  emit('written');
}

const focusById = (id: string): void => void nextTick(() => document.getElementById(id)?.focus());

function askDeleteAll(): void {
  confirming.value = true;
  focusById('ed-confirm-no');
}

function answerDeleteAll(yes: boolean): void {
  if (yes) void write('delete', 'all');
  else confirming.value = false;
  focusById('ed-delete-all');
}

function save(): void {
  if (adding.value) void write('set', target.value === 'all' ? 'all' : Number(target.value));
  else if (props.view) void write('set', props.view.id);
}

async function togglePin(): Promise<void> {
  if (!props.row || !props.view) return;
  try {
    const answer = (pinned.value
      ? await panel.call({ command: 'panel' }, { op: 'unpin', id: pinned.value.id })
      : await panel.call({ command: 'panel' }, { op: 'pin', sessionOnly: sessionOnly.value, ...entryArgs(props.kind, props.row, props.view.id) })) as { pins?: StoragePin[] } | null;
    emit('pins', answer?.pins ?? []);
    emit('close');
    emit('written');
  } catch (error) {
    status.value = t('panel.error', { text: panel.error(error) });
  }
}

function onKey(event: KeyboardEvent): void {
  if (event.key !== 'Escape') return;
  event.preventDefault();
  emit('close');
}

onMounted(async () => {
  await nextTick();
  if (adding.value) nameField.value?.inputRef?.focus();
  else value.value?.focus();
  if (props.row && props.view && props.row.cells[String(props.view.id)]) {
    try {
      const full = (await panel.call({ command: 'panel' }, { op: 'value', ...entryArgs(props.kind, props.row, props.view.id) })) as { value?: string } | null;
      value.value?.set(full?.value ?? '');
    } catch (error) {
      status.value = t('panel.error', { text: panel.error(error) });
    }
  }
});
</script>

<template>
  <UCard
    id="editor"
    as="section"
    :data-mode="adding ? 'add' : 'edit'"
    aria-labelledby="editor-title"
    :ui="{ header: 'flex items-center gap-2 px-3 py-2 sm:px-3', body: 'flex flex-col gap-3 p-3 sm:p-3' }"
    @keydown="onKey"
  >
    <template #header>
      <h2 id="editor-title" class="min-w-0 flex-1 truncate text-sm font-semibold text-highlighted">
        {{ adding ? t('panel.addTitle') : t('panel.editTitle', { name: row?.name, view: view?.label }) }}
      </h2>
      <IconAction id="ed-close" icon="i-lucide-x" :label="t('panel.close')" @click="emit('close')" />
    </template>

    <div class="grid gap-2 sm:grid-cols-2">
      <UFormField :label="t('panel.name')">
        <UInput id="ed-name" ref="nameField" v-model="form.name" :readonly="!adding" :maxlength="4096" autocomplete="off" :spellcheck="false" class="w-full" />
      </UFormField>
      <UFormField :label="t('panel.scope')">
        <UInput id="ed-scope" v-model="form.scope" :readonly="!adding" :maxlength="300" autocomplete="off" :spellcheck="false" class="w-full" />
      </UFormField>
    </div>
    <div class="flex flex-col gap-1">
      <label for="ed-value" class="text-sm font-medium text-default">{{ t('panel.value') }}</label>
      <ValueField id="ed-value" ref="value" />
    </div>

    <fieldset v-if="kind === 'cookies'" id="ed-cookie" class="grid gap-2 rounded-md border border-default p-2 sm:grid-cols-3">
      <legend class="px-1 text-xs font-semibold uppercase text-toned">{{ t('panel.attributes') }}</legend>
      <UFormField :label="t('panel.path')">
        <UInput id="ed-path" v-model="form.path" :maxlength="1024" autocomplete="off" :spellcheck="false" class="w-full" />
      </UFormField>
      <UFormField :label="t('panel.expires')">
        <UInput id="ed-expires" v-model="form.expires" placeholder="2030-01-01T00:00:00Z" autocomplete="off" :spellcheck="false" class="w-full" />
      </UFormField>
      <UFormField :label="t('panel.sameSite')">
        <USelect id="ed-samesite" v-model="sameSite" :items="sameSites" class="w-full" />
      </UFormField>
      <UCheckbox id="ed-httponly" v-model="form.httpOnly" :label="t('panel.httpOnly')" />
      <UCheckbox id="ed-secure" v-model="form.secure" :label="t('panel.secure')" />
    </fieldset>

    <UFormField v-if="adding" id="ed-target-row" :label="t('panel.target')">
      <USelect id="ed-target" v-model="target" :items="targets" class="w-full sm:w-64" />
    </UFormField>

    <div class="flex flex-wrap items-center gap-2">
      <UButton id="ed-save" color="primary" icon="i-lucide-save" :label="t('panel.save')" @click="save" />
      <template v-if="!adding">
        <UButton id="ed-all" color="neutral" variant="outline" icon="i-lucide-copy-check" :label="t('panel.applyAll')" @click="write('set', 'all')" />
        <UButton id="ed-delete" color="error" variant="outline" icon="i-lucide-trash-2" :label="t('panel.delete')" @click="view && write('delete', view.id)" />
        <UButton id="ed-delete-all" color="error" variant="outline" icon="i-lucide-trash" :label="t('panel.deleteAll')" @click="askDeleteAll" />
        <UButton id="ed-pin" color="neutral" variant="outline" :icon="pinned ? 'i-lucide-pin-off' : 'i-lucide-pin'" :label="pinned ? t('panel.unpin') : t('panel.pin')" @click="togglePin" />
        <UCheckbox id="ed-session-only" v-model="sessionOnly" :label="t('panel.sessionOnly')" />
      </template>
    </div>
    <UAlert
      v-if="confirming"
      id="ed-confirm"
      color="error"
      variant="subtle"
      icon="i-lucide-triangle-alert"
      :title="t('panel.deleteAllConfirm', { name: row?.name ?? '', count: okViews })"
      :actions="[
        { label: t('panel.deleteAllYes'), color: 'error', id: 'ed-confirm-yes', onClick: () => answerDeleteAll(true) },
        { label: t('panel.cancel'), color: 'neutral', variant: 'outline', id: 'ed-confirm-no', onClick: () => answerDeleteAll(false) },
      ]"
      :ui="{ root: 'px-3 py-2', title: 'text-sm font-normal' }"
    />
    <p id="ed-status" role="status" class="min-h-5 text-sm text-muted">{{ status }}</p>
  </UCard>
</template>
