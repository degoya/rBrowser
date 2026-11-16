<script setup lang="ts">
// One section's entries as Nuxt UI's tree (RB-080-85): nested by depth and open, the keyboard of
// the WAI-ARIA tree pattern (reka-ui), a dot per view (visible, hidden, missing) with its words as
// the accessible name, "only <views>" and the findings as badges coloured by severity, the link check's state at a
// link. Selecting an entry (a click, Enter or Space) marks it in every view; the tree keeps no
// selection of its own.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Item, nest, type Presence, SEVERITY_ICON, text, type TreeNode } from './model';

const props = defineProps<{ items: Item[]; links: Map<string, { state: string; status?: number }> }>();
const emit = defineEmits<{ activate: [item: Item] }>();
const { strings } = usePanel();

const tree = computed(() => nest(props.items));

const DOT: Record<Presence, string> = {
  visible: 'bg-success',
  hidden: 'bg-transparent ring-1 ring-inset ring-muted',
  missing: 'bg-error/70',
};
const BROKEN = new Set(['broken', 'failed', 'redirect-error']);

function linkOf(item: Item): { state: string; status?: number } | undefined {
  return item.href ? props.links.get(item.href.split('#')[0] ?? item.href) : undefined;
}

function onSelect(event: Event, node: TreeNode): void {
  // The tree keeps no selection: an entry is marked, not chosen.
  event.preventDefault();
  emit('activate', node.item);
}
</script>

<template>
  <UTree
    id="tree"
    :items="tree"
    :get-key="(node: TreeNode) => node.key"
    size="sm"
    color="neutral"
    :ui="{ linkLabel: 'flex min-w-0 flex-wrap items-center gap-1.5 whitespace-normal text-left' }"
    @select="(event: Event, node: TreeNode) => onSelect(event, node)"
  >
    <template #item-label="{ item }">
      <span v-if="(item as TreeNode).item.presence.length > 0" data-dots class="inline-flex shrink-0 items-center gap-0.5">
        <span
          v-for="one in (item as TreeNode).item.presence"
          :key="one.view.id"
          role="img"
          :data-state="one.state"
          :aria-label="text(strings, 'panel.dot', { view: one.view.label, state: text(strings, `panel.state.${one.state}`) })"
          :title="text(strings, 'panel.dot', { view: one.view.label, state: text(strings, `panel.state.${one.state}`) })"
          class="size-2 rounded-full"
          :class="DOT[one.state]"
        />
      </span>
      <span data-label class="min-w-0 break-all text-default">{{ (item as TreeNode).label }}</span>
      <UBadge v-if="(item as TreeNode).item.badge" data-badge color="neutral" variant="subtle" size="sm" :label="(item as TreeNode).item.badge" />
      <UBadge
        v-for="note in (item as TreeNode).item.notes"
        :key="note.text"
        data-note
        :data-severity="note.severity"
        :color="note.severity"
        variant="subtle"
        size="sm"
        :icon="SEVERITY_ICON[note.severity]"
        :label="note.text"
      />
      <UBadge
        v-if="linkOf((item as TreeNode).item)"
        data-link
        :data-state="linkOf((item as TreeNode).item)?.state"
        :color="BROKEN.has(linkOf((item as TreeNode).item)?.state ?? '') ? 'error' : linkOf((item as TreeNode).item)?.state === 'ok' ? 'success' : 'neutral'"
        variant="subtle"
        size="sm"
        :label="text(strings, `panel.link.${linkOf((item as TreeNode).item)?.state}`, { status: linkOf((item as TreeNode).item)?.status ?? '' })"
      />
    </template>
  </UTree>
</template>
