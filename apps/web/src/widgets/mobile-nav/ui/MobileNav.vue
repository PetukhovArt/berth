<script setup lang="ts">
import { Eye, Inbox, Settings, SquareTerminal } from '../../../shared/ui/icons';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { VRowButton } from '../../../shared/ui/kit';
import { openPhoneSection, phoneSection, type PhoneSection } from '../model/section';

const { t } = useI18n();

const ITEMS = [
  { section: 'inbox', icon: Inbox },
  { section: 'sessions', icon: SquareTerminal },
  { section: 'reviews', icon: Eye },
  { section: 'settings', icon: Settings },
] as const satisfies readonly { section: PhoneSection; icon: unknown }[];
</script>

<template>
  <nav class="nav" :aria-label="t('mobile.navLabel')" data-testid="mobile-nav">
    <VRowButton
      v-for="item in ITEMS"
      :key="item.section"
      stacked
      class="item"
      :active="phoneSection === item.section"
      :aria-current="phoneSection === item.section ? 'page' : undefined"
      :data-testid="`mobile-nav-${item.section}`"
      @click="openPhoneSection(item.section)"
    >
      <template #leading><component :is="item.icon" :size="20" /></template>
      {{ t(`mobile.section.${item.section}`) }}
    </VRowButton>
  </nav>
</template>

<style scoped>
.nav {
  display: flex;
  flex: none;
  padding: var(--vr-space-1) var(--vr-space-1) calc(var(--vr-space-1) + var(--vr-safe-bottom));
  background: var(--vr-color-surface);
  border-top: 1px solid var(--vr-color-border);
}

.item {
  flex: 1;
}

@media (orientation: landscape) {
  .nav {
    flex-direction: column;
    justify-content: flex-start;
    gap: var(--vr-space-1);
    width: 4.5rem;
    padding: var(--vr-space-2) var(--vr-space-1) calc(var(--vr-space-1) + var(--vr-safe-bottom));
    border-top: 0;
    border-right: 1px solid var(--vr-color-border);
  }

  .item {
    flex: none;
    min-height: var(--vr-size-touch);
  }
}
</style>
