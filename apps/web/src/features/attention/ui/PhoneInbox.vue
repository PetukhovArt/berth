<script setup lang="ts">
import { computed } from 'vue';
import { Inbox } from '../../../shared/ui/icons';
import { useChannelUnread } from '../../../entities/session';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { useInbox } from '../model/inbox';
import InboxSection from './InboxSection.vue';

const { t } = useI18n();
const { items } = useInbox();
const { unread } = useChannelUnread();
const empty = computed(() => !items.value.length && !unread.value.length);
</script>

<template>
  <section class="phone-inbox" data-testid="phone-inbox">
    <InboxSection />
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
