<script setup lang="ts">
import { useDock } from '../../../shared/model/dock';
import SearchPalette from './SearchPalette.vue';

const { switching } = useDock();
</script>

<template>
  <Transition name="qs">
    <SearchPalette v-if="switching" @close="switching = false" />
  </Transition>
</template>

<style scoped>
.qs-enter-active {
  transition: opacity var(--vr-motion-overlay-in) var(--vr-motion-ease-standard);
}

.qs-leave-active {
  transition: opacity var(--vr-motion-overlay-out) var(--vr-motion-ease-accelerate);
}

.qs-enter-active :deep(.panel) {
  transition: transform var(--vr-motion-overlay-in) var(--vr-motion-ease-standard);
}

.qs-enter-from,
.qs-leave-to {
  opacity: 0;
}

.qs-enter-from :deep(.panel) {
  transform: scale(0.98);
}
</style>
