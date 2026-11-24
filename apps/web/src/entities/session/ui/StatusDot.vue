<script setup lang="ts">
import { computed } from 'vue';
import { useHollow } from '../lib/hollow';
import { isStopped } from '../lib/liveness';
import type { SessionStatus } from '../model/berth';
import StatusIcon from './StatusIcon.vue';

const props = defineProps<{ status: SessionStatus | undefined; unseen: boolean | undefined; alive?: boolean | undefined }>();

const hollow = useHollow();
const stopped = computed(() => isStopped(props.status, props.unseen, props.alive));
</script>

<template>
  <span class="status-dot" :data-status="hollow ? 'disconnected' : stopped ? 'stopped' : status" :data-unseen="hollow ? false : unseen" :data-alive="!hollow && alive === true">
    <StatusIcon :status="status" :unseen="unseen" :stopped="stopped" />
    <span v-if="alive === true && !hollow" class="live" data-testid="status-live" />
  </span>
</template>

<style scoped>
.status-dot {
  position: relative;
  display: inline-flex;
  flex: none;
  border-radius: 50%;
}

.live {
  position: absolute;
  right: -2px;
  bottom: -2px;
  width: 6px;
  height: 6px;
  background: var(--vr-color-presence-online);
  border-radius: 50%;
  box-shadow: 0 0 0 1.5px var(--vr-color-surface);
}
</style>
