<script setup lang="ts">
defineProps<{ label: string }>();
</script>

<template>
  <div class="cell">
    <slot />
    <span class="cell-label">{{ label }}</span>
  </div>
</template>
