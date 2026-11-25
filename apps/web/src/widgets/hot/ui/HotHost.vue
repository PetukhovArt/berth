<script setup lang="ts">
import { onBeforeUnmount } from 'vue';
import { installHotkeys, switcher } from '../model/hotkeys';
import HotSwitcher from './HotSwitcher.vue';

const uninstall = installHotkeys();
onBeforeUnmount(uninstall);
</script>

<template>
  <Transition name="hs">
    <HotSwitcher v-if="switcher" />
  </Transition>
</template>
