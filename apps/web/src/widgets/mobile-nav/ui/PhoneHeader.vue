<script setup lang="ts">
import { computed } from 'vue';
import { ChevronLeft, Command, Plus } from '../../../shared/ui/icons';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { hostText } from '../../../shared/lib/i18n/hostText';
import { paletteOpen } from '../../../shared/lib/commands/context';
import { useDock } from '../../../shared/model/dock';
import { view } from '../../../shared/model/nav';
import { isRunning, useBerth, useSessionDock, useSessionStates } from '../../../entities/session';
import { useInbox } from '../../../features/attention';
import { startNewSession } from '../../../features/new-session';
import { ConnectorErrors, useWorkTree } from '../../../features/work-tree';
import { VButton, vTooltip } from '../../../shared/ui/kit';
import BrandMark from '../../../shared/ui/BrandMark.vue';
import { phoneSection } from '../model/section';

const { t } = useI18n();
const berth = useBerth();
const { detail, activeTitle } = useDock();
const { activeSession } = useSessionDock();
const { items } = useInbox();
const { tabs } = useWorkTree();

const inDetail = computed(() => view.value.kind === 'agents' && detail.value);
const session = computed(() => (inDetail.value ? activeSession.value : undefined));
const states = useSessionStates();
const running = computed(() => [...states.value.values()].filter(isRunning).length);
const reviews = computed(() => tabs.value.find((tab) => tab.id === 'reviews')?.count ?? 0);

const offline = computed(() => {
  const connection = berth?.connection.value;
  if (!connection || connection.state === 'connected') return null;
  if (connection.state === 'incompatible') return hostText(connection.message) ?? t('agents.notices.incompatible');
  return t(`agents.notices.${connection.state}`);
});

const title = computed(() => {
  if (inDetail.value) return session.value?.name ?? activeTitle.value;
  if (phoneSection.value) return t(`mobile.section.${phoneSection.value}`);
  return view.value.kind === 'jira' ? t('shell.nav.jira') : view.value.kind === 'confluence' ? t('shell.nav.confluence') : '';
});

const summary = computed(() => {
  if (inDetail.value) return '';
  switch (phoneSection.value) {
    case 'inbox': {
      const parts = [items.value.length ? t('mobile.summary.needYou', { count: items.value.length }) : '', running.value ? t('mobile.summary.running', { count: running.value }) : ''].filter(Boolean);
      return parts.length ? parts.join(' · ') : t('mobile.summary.quiet');
    }
    case 'sessions':
      return running.value ? t('mobile.summary.running', { count: running.value }) : '';
    case 'reviews':
      return reviews.value ? t('mobile.summary.reviews', { count: reviews.value }) : '';
    default:
      return '';
  }
});

const canCreate = computed(() => !inDetail.value && (phoneSection.value === 'inbox' || phoneSection.value === 'sessions') && !!berth);
</script>

<template>
  <header v-if="!session" class="phone-header" data-testid="phone-header">
    <VButton v-if="inDetail" v-tooltip="t('mobile.back')" class="icon back" text severity="secondary" :aria-label="t('mobile.back')" data-testid="mobile-back" @click="detail = false">
      <template #icon><ChevronLeft :size="22" /></template>
    </VButton>
    <BrandMark v-else size="1.75rem" data-testid="phone-header-mark" />
    <div class="identity">
      <h1 class="title" data-testid="phone-header-title">{{ title }}</h1>
      <p v-if="offline" class="sub" data-tone="offline" role="status" data-testid="phone-header-connection">{{ offline }}</p>
      <p v-else-if="summary" class="sub" data-testid="phone-header-summary">{{ summary }}</p>
    </div>
    <ConnectorErrors />
    <VButton v-tooltip="t('mobile.commands')" class="icon" text severity="secondary" :aria-pressed="paletteOpen" :aria-label="t('mobile.commands')" data-testid="phone-header-commands" @click="paletteOpen = !paletteOpen">
      <template #icon><Command :size="18" /></template>
    </VButton>
    <VButton v-if="canCreate" v-tooltip="t('agents.newSession.action')" class="icon" severity="secondary" :aria-label="t('agents.newSession.action')" data-testid="phone-header-new" @click="startNewSession()">
      <template #icon><Plus :size="20" /></template>
    </VButton>
  </header>
</template>

<style scoped>
.phone-header {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  min-height: 3.5rem;
  padding: var(--vr-space-2) var(--vr-page-gutter);
  background: var(--vr-color-canvas);
  border-bottom: 1px solid var(--vr-color-border);
}


.identity {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.title {
  margin: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-lg);
  font-weight: 600;
  line-height: 1.25;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.sub {
  margin: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.sub[data-tone='offline'] {
  color: var(--vr-status-limit);
}

.icon {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: var(--vr-size-touch);
  height: var(--vr-size-touch);
  padding: 0;
}

.icon[aria-pressed='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
}

.back {
  margin-left: calc(var(--vr-space-2) * -1);
  color: var(--vr-color-text);
}
</style>
