<script setup lang="ts">
import UApp from '@nuxt/ui/components/App.vue';
import ShellToolbar from './toolbar/ShellToolbar.vue';
import WhichKeyBar from './whichkey/WhichKeyBar.vue';
import SizeInputBar from './breakpoint_keys/SizeInputBar.vue';
import NewSizeBar from './toolbar/NewSizeBar.vue';
import SessionBar from './sessions/SessionBar.vue';
import BookmarkBar from './bookmarks/BookmarkBar.vue';
import SitemapBar from './sitemap/SitemapBar.vue';
import SavePresetBar from './layout/SavePresetBar.vue';
import ExtensionPromptBar from './extensions/ExtensionPromptBar.vue';
import ActivityBar from './activity/ActivityBar.vue';
import { useActivities } from './activity/store';
// The command palette and the findings panel live in windows of their own (the pages,
// `palette`, `findings`): the shell strip keeps its 44 px. The activity bar below the toolbar
// (RB-090-67) takes its 28 px from the views only while something runs: this page asks the core
// for the room (`manageRoom`), for the bar above the status bar too.
useActivities().manageRoom();
</script>

<template>
  <UApp>
    <ShellToolbar />
    <ActivityBar place="top" />
    <WhichKeyBar />
    <SizeInputBar />
    <NewSizeBar />
    <SessionBar />
    <BookmarkBar />
    <SitemapBar />
    <SavePresetBar />
    <ExtensionPromptBar />
  </UApp>
</template>
