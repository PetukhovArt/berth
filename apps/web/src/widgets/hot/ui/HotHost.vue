<script setup lang="ts">
import { onBeforeUnmount } from 'vue';
import { installHotkeys, switcher } from '../model/hotkeys';
import HotOverview from './HotOverview.vue';
import HotSwitcher from './HotSwitcher.vue';
import { hotEnabled } from '../model/view';

const uninstall = installHotkeys();
onBeforeUnmount(uninstall);
</script>

<template>
  <HotOverview v-if="hotEnabled" />
  <Transition name="hs">
    <HotSwitcher v-if="switcher" />
  </Transition>
</template>
