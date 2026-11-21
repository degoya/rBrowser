<script setup lang="ts">
// The checks of a request (RB-096-04): one row per check - its kind, then what it compares -,
// kept as `# @rb-check` lines in the request's file. A row not of its form is marked; the core
// refuses to save or send it. Values are compared as written, never as variables.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import { CHECK_KINDS, COMPARES, type Check, type CheckKind, type Compare, MAX_BELOW_MS, MAX_CHECKS, checkOk, newCheck } from './checks';

const checks = defineModel<Check[]>({ required: true });
/** The fields' examples and the time's sign: a check's form, not a text to translate. */
const STATUS_EXAMPLE = '2xx';
const PATH_EXAMPLE = '$.user.id';
const BELOW = '<';
const t = usePanel().t;

const kindItems = computed(() => CHECK_KINDS.map((kind) => ({ label: t(`panel.checks.kinds.${kind}`), value: kind })));
const compareItems = computed(() => COMPARES.map((compare) => ({ label: t(`panel.checks.compares.${compare}`), value: compare })));

function replace(index: number, next: Check): void {
  checks.value = checks.value.map((check, at) => (at === index ? next : check));
}

function patch(index: number, fields: Record<string, unknown>): void {
  const check = checks.value[index];
  if (check) replace(index, { ...check, ...fields } as Check);
}

function setKind(index: number, kind: unknown): void {
  const check = checks.value[index];
  if (check && check.kind !== kind && CHECK_KINDS.includes(kind as CheckKind)) replace(index, newCheck(kind as CheckKind));
}

function setBelow(index: number, text: string | number): void {
  const ms = Number(text);
  if (Number.isFinite(ms)) patch(index, { belowMs: Math.min(Math.max(Math.round(ms), 1), MAX_BELOW_MS) });
}

function add(): void {
  if (checks.value.length < MAX_CHECKS) checks.value = [...checks.value, newCheck('status')];
}

function remove(index: number): void {
  checks.value = checks.value.filter((_, at) => at !== index);
}
</script>

<template>
  <div id="checks" class="flex flex-col gap-1.5 pt-2">
    <p class="text-xs text-muted">{{ t('panel.checks.hint') }}</p>
    <div v-for="(check, index) in checks" :key="index" data-check class="flex flex-wrap items-center gap-1.5">
      <USelect
        :model-value="check.kind"
        :items="kindItems"
        value-key="value"
        size="sm"
        class="w-32"
        :aria-label="t('panel.checks.kind')"
        @update:model-value="(kind) => setKind(index, kind)"
      />
      <UInput
        v-if="check.kind === 'status'"
        :model-value="check.expected"
        size="sm"
        class="w-28 font-mono"
        maxlength="3"
        :placeholder="STATUS_EXAMPLE"
        :color="checkOk(check) ? undefined : 'error'"
        :aria-invalid="!checkOk(check)"
        :aria-label="t('panel.checks.expected')"
        @update:model-value="(text) => patch(index, { expected: String(text).trim() })"
      />
      <template v-else-if="check.kind === 'json' || check.kind === 'header'">
        <UInput
          v-if="check.kind === 'json'"
          :model-value="check.path"
          size="sm"
          class="min-w-0 flex-1 font-mono"
          spellcheck="false"
          :placeholder="PATH_EXAMPLE"
          :color="checkOk(check) ? undefined : 'error'"
          :aria-invalid="!checkOk(check)"
          :aria-label="t('panel.checks.path')"
          @update:model-value="(text) => patch(index, { path: String(text) })"
        />
        <UInput
          v-else
          :model-value="check.name"
          size="sm"
          class="min-w-0 flex-1 font-mono"
          spellcheck="false"
          :color="checkOk(check) ? undefined : 'error'"
          :aria-invalid="!checkOk(check)"
          :aria-label="t('panel.checks.header')"
          @update:model-value="(text) => patch(index, { name: String(text).trim() })"
        />
        <USelect
          :model-value="check.compare"
          :items="compareItems"
          value-key="value"
          size="sm"
          class="w-28"
          :aria-label="t('panel.checks.compare')"
          @update:model-value="(compare) => patch(index, { compare: compare as Compare })"
        />
        <UInput
          :model-value="check.value"
          size="sm"
          class="min-w-0 flex-1 font-mono"
          spellcheck="false"
          :aria-label="t('panel.checks.value')"
          @update:model-value="(text) => patch(index, { value: String(text) })"
        />
      </template>
      <UInput
        v-else
        type="number"
        :model-value="check.belowMs"
        size="sm"
        class="w-32"
        min="1"
        :max="MAX_BELOW_MS"
        :aria-label="t('panel.checks.belowMs')"
        @update:model-value="(text) => setBelow(index, text)"
      >
        <template #leading>
          <span class="text-xs text-muted">{{ BELOW }}</span>
        </template>
      </UInput>
      <IconAction icon="i-lucide-x" :label="t('panel.checks.remove')" @click="remove(index)" />
    </div>
    <div>
      <UButton id="add-check" size="xs" color="neutral" variant="ghost" icon="i-lucide-plus" :label="t('panel.checks.add')" :disabled="checks.length >= MAX_CHECKS" @click="add" />
    </div>
  </div>
</template>
