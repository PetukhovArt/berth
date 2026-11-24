<script setup lang="ts">
import { computed } from 'vue';
import { Inbox } from '../../../shared/ui/icons';
import { isRunning, useChannelUnread, useSessionDock, useSessionStates } from '../../../entities/session';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { UiButton } from '../../../shared/ui/UiButton';
import { useInbox } from '../model/inbox';
import InboxSection from './InboxSection.vue';

const { t } = useI18n();
const { items } = useInbox();
const { unread } = useChannelUnread();
const empty = computed(() => !items.value.length && !unread.value.length);
const states = useSessionStates();
const dock = useSessionDock();
const running = computed(() => [...states.value.values()].filter(isRunning));
const runningLine = computed(() => (running.value.length ? t('mobile.runningLine', { count: running.value.length, names: running.value.map((state) => state.name).join(', ') }) : ''));
</script>

<template>
  <section class="phone-inbox" data-testid="phone-inbox">
    <InboxSection />
    <UiButton v-if="runningLine" type="button" class="running" data-testid="phone-inbox-running" @click="dock.openKey(running[0]!.key)">{{ runningLine }}</UiButton>
    <p v-if="empty" class="empty" data-testid="phone-inbox-empty"><Inbox :size="22" />{{ t('mobile.inboxEmpty') }}</p>
  </section>
</template>

<style scoped>
.phone-inbox {
  flex: 1;
  min-height: 0;
  padding: var(--vr-space-3) var(--vr-page-gutter) var(--vr-space-4);
  overflow-y: auto;
  overscroll-behavior-y: contain;
}

.running {
  display: block;
  width: 100%;
  padding: var(--vr-space-2) var(--vr-space-1);
  overflow: hidden;
  font: inherit;
  font-size: var(--vr-font-size-label);
  color: var(--vr-color-text-muted);
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
  background: none;
  border: 0;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--vr-space-2);
  margin: var(--vr-space-8) 0 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
  text-align: center;
}
</style>
