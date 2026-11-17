<script setup lang="ts">
import type { SessionStatus } from '../model/berth';
import StatusIcon from './StatusIcon.vue';

defineProps<{ status: SessionStatus | undefined; unseen: boolean | undefined }>();
</script>

<template>
  <span class="status-dot" :data-status="status" :data-unseen="unseen">
    <StatusIcon :status="status" :unseen="unseen" />
  </span>
</template>

<style scoped>
.status-dot {
  display: inline-flex;
  flex: none;
  border-radius: 50%;
}
</style>
