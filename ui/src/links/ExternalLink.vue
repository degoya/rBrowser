<script setup lang="ts">
// A link out of rBrowser (RB-080-92) - the shell's pages allow no popup, so it never navigates
// itself: a click hands the address to the core's `app.open_link` (`opener.ts`), which opens it
// in the system's browser or the active view as `ui.openLinksIn` says, after asking for `ask`.
// Only a web address is a link; anything else shows as text. Every external link of `ui/src`
// is this component (`links.test.ts` refuses a bare blank target).
import { computed } from 'vue';
import ULink from '@nuxt/ui/components/Link.vue';
import LinkChooser from './LinkChooser.vue';
import { isOpenableUrl } from './open';
import { useLinkOpener } from './opener';

// Two roots (the link and its chooser): the caller's class goes to the link.
defineOptions({ inheritAttrs: false });
const props = defineProps<{ href: string }>();
const opener = useLinkOpener();
const openable = computed(() => isOpenableUrl(props.href));

function follow(): void {
  void opener.open(props.href);
}
</script>

<template>
  <ULink v-if="openable" v-bind="$attrs" :href="href" target="_self" data-external-link @click.prevent="follow">
    <slot />
  </ULink>
  <span v-else v-bind="$attrs"><slot /></span>
  <LinkChooser v-if="openable" :opener="opener" />
</template>
