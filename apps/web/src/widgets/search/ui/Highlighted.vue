<script setup lang="ts">
import type { Segment } from '../../../shared/lib/segments';

defineProps<{ segments: readonly Segment[] }>();
</script>

<template>
  <span class="highlighted"><template v-for="(segment, at) in segments" :key="at"><mark v-if="segment.match">{{ segment.text }}</mark><template v-else>{{ segment.text }}</template></template></span>
</template>

<style scoped>
mark {
  color: var(--vr-color-match-text);
  background: var(--vr-color-match-bg);
  border-radius: var(--vr-radius-sm);
}
</style>
