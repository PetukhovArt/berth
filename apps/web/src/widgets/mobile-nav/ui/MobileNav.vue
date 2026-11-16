<script setup lang="ts">
import { Eye, Inbox, Settings, SquareTerminal } from '../../../shared/ui/icons';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { UiButton } from '../../../shared/ui/UiButton';
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
    <UiButton
      v-for="item in ITEMS"
      :key="item.section"
      class="item"
      :data-active="phoneSection === item.section"
      :aria-current="phoneSection === item.section ? 'page' : undefined"
      :data-testid="`mobile-nav-${item.section}`"
      @click="openPhoneSection(item.section)"
    >
      <component :is="item.icon" :size="20" />
      <span>{{ t(`mobile.section.${item.section}`) }}</span>
    </UiButton>
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
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: var(--vr-space-1) 0;
  font: inherit;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
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

.item[data-active='true'] {
  color: var(--vr-color-text);
}
</style>
