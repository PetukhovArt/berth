<script setup lang="ts">
defineProps<{ id: string; title: string; base: string; adds: string; candidate?: boolean }>();
</script>

<template>
  <section :id="id" class="sec">
    <h2 class="sec-title">
      <span>{{ title }}</span>
      <span class="base">· {{ base }}</span>
    </h2>
    <p class="sec-note" :class="{ candidate }">{{ adds }}</p>
    <slot />
  </section>
</template>
