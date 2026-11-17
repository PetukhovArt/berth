<script setup lang="ts">
defineProps<{ size: string }>();
</script>

<template>
  <img class="brand-mark" src="/favicon.svg" alt="" aria-hidden="true" draggable="false" :style="{ width: size, height: size }" />
</template>

<style scoped>
.brand-mark {
  display: block;
  flex: none;
  pointer-events: none;
  user-select: none;
}
</style>
