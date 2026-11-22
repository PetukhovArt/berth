<script setup lang="ts">
import { useI18n } from '../../../shared/lib/i18n/i18n';
import type { HubAdmin } from '../../../entities/session';
import { auditSentence } from '../lib/auditSentence';

defineProps<{ admin: HubAdmin }>();

const SECONDS_LIMIT = 1e11;
const { t, formatDate } = useI18n();

const millis = (value: number) => (value < SECONDS_LIMIT ? value * 1000 : value);
</script>

<template>
  <section class="audit" data-testid="hub-admin-audit">
    <h3 class="head">{{ t('hub.audit.title') }}</h3>
    <p v-if="!admin.audit.length" class="muted" data-testid="hub-admin-audit-empty">{{ t('hub.audit.empty') }}</p>
    <ol v-else class="list" data-testid="hub-admin-audit-list">
      <li v-for="entry in admin.audit" :key="entry.seq" class="entry" :data-testid="`hub-admin-audit-${entry.seq}`">
        <time class="time">{{ formatDate(millis(entry.at)) }}</time>
        <span class="text">{{ auditSentence(entry, admin) }}</span>
      </li>
    </ol>
  </section>
</template>

<style scoped>
.audit {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  min-width: 0;
}

.head {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.muted {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.list {
  display: flex;
  flex-direction: column;
  max-height: 24rem;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.entry {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-1) var(--vr-space-3);
  padding: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-sm);
}

.entry + .entry {
  border-top: 1px solid var(--vr-color-border);
}

.time {
  flex: none;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.text {
  flex: 1 1 16rem;
  min-width: 0;
  overflow-wrap: anywhere;
}
</style>
