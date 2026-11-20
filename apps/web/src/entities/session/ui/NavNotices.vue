<script setup lang="ts">
import { computed } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { hostText } from '../../../shared/lib/i18n/hostText';
import { useBerth } from '../model/berth';

const berth = useBerth();

const banner = computed(() => {
  const connection = berth?.connection.value;
  if (!connection) return null;
  switch (connection.state) {
    case 'connected':
      return null;
    case 'connecting':
      return t('agents.notices.connecting');
    case 'offline':
      return t('agents.notices.offline');
    case 'incompatible':
      return hostText(connection.message) ?? t('agents.notices.incompatible');
    default: {
      const unknown: never = connection.state;
      return unknown;
    }
  }
});

const detail = computed(() => {
  const connection = berth?.connection.value;
  return connection?.state === 'offline' ? (hostText(connection.message) ?? null) : null;
});
</script>

<template>
  <template v-if="berth">
    <p v-if="banner" class="banner" role="status" data-testid="berth-banner" :data-state="berth.connection.value.state">
      {{ banner }}
      <span v-if="detail" class="detail" data-testid="berth-banner-detail">{{ detail }}</span>
    </p>
    <p v-if="berth.error.value" class="banner" role="alert" data-testid="berth-error">{{ hostText(berth.error.value) }}</p>
  </template>
</template>

<style scoped>
.banner {
  margin: 0 0 var(--vr-space-2);
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  border-radius: var(--vr-radius-md);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
}

.detail {
  display: block;
  margin-top: var(--vr-space-1);
  color: var(--vr-color-text-muted);
  overflow-wrap: anywhere;
}

.banner[role='alert'],
.banner[data-state='offline'],
.banner[data-state='incompatible'] {
  color: var(--vr-color-danger);
}
</style>
