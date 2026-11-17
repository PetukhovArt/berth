<script setup lang="ts">
import { computed } from 'vue';
import { ChevronLeft, Command, Plus } from '../../../shared/ui/icons';
import { useI18n } from '../../../shared/lib/i18n/i18n';
import { paletteOpen } from '../../../shared/lib/commands/context';
import { useDock } from '../../../shared/model/dock';
import { view } from '../../../shared/model/nav';
import { StatusDot, useBerth, useSessionDock, useSessionStates, type SessionStatus } from '../../../entities/session';
import { useInbox } from '../../../features/attention';
import { startNewSession } from '../../../features/new-session';
import { useWorkTree } from '../../../features/work-tree';
import { UiButton } from '../../../shared/ui/UiButton';
import BrandMark from '../../../shared/ui/BrandMark.vue';
import { phoneSection } from '../model/section';

const { t, formatTime } = useI18n();
const berth = useBerth();
const { detail, activeTitle } = useDock();
const { activeSession } = useSessionDock();
const { items } = useInbox();
const { tabs } = useWorkTree();

const TONES: Partial<Record<SessionStatus, string>> = { running: 'running', needs_feedback: 'feedback', terminated: 'crashed', limit: 'limit' };

const inDetail = computed(() => view.value.kind === 'agents' && detail.value);
const session = computed(() => (inDetail.value ? activeSession.value : undefined));
const states = useSessionStates();
const state = computed(() => (session.value ? states.value.get(session.value.key) : undefined));
const running = computed(() => [...states.value.values()].filter((entry) => entry.status === 'running').length);
const reviews = computed(() => tabs.value.find((tab) => tab.id === 'reviews')?.count ?? 0);

const offline = computed(() => {
  const connection = berth?.connection.value;
  if (!connection || connection.state === 'connected') return null;
  if (connection.state === 'incompatible') return connection.message ?? t('agents.notices.incompatible');
  return t(`agents.notices.${connection.state}`);
});

const title = computed(() => {
  if (inDetail.value) return session.value?.name ?? activeTitle.value;
  if (phoneSection.value) return t(`mobile.section.${phoneSection.value}`);
  return view.value.kind === 'jira' ? t('shell.nav.jira') : view.value.kind === 'confluence' ? t('shell.nav.confluence') : '';
});

const place = computed(() => {
  const ref = session.value?.ref;
  const entities = berth?.entities.value;
  if (!ref || !entities) return '';
  const worktreeId = 'Agent' in ref ? entities.agents.get(ref.Agent)?.worktree_id : entities.terminals.get(ref.Terminal)?.worktree_id;
  const worktree = worktreeId ? entities.worktrees.get(worktreeId) : undefined;
  const project = worktree ? entities.projects.get(worktree.project_id) : undefined;
  return [project?.name, worktree?.branch].filter(Boolean).join(' › ');
});

const statusText = computed(() => {
  const node = session.value;
  if (!node) return '';
  const status = state.value?.status ?? node.status;
  const until = state.value?.limitUntil;
  const label = t(`agents.status.${status}`);
  return until ? `${label} · ${t('agents.metrics.resets', { time: formatTime(until) })}` : label;
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
  <header class="phone-header" data-testid="phone-header">
    <UiButton v-if="inDetail" type="button" class="icon back" :aria-label="t('mobile.back')" :title="t('mobile.back')" data-testid="mobile-back" @click="detail = false"><ChevronLeft :size="22" /></UiButton>
    <BrandMark v-else size="1.75rem" data-testid="phone-header-mark" />
    <div class="identity">
      <h1 class="title" data-testid="phone-header-title">{{ title }}</h1>
      <p v-if="offline" class="sub" data-tone="offline" role="status" data-testid="phone-header-connection">{{ offline }}</p>
      <p v-else-if="session" class="sub" data-testid="phone-header-status">
        <StatusDot class="status-icon" :status="state?.status ?? session.status" :unseen="session.unseen" />
        <span class="status" :data-tone="TONES[state?.status ?? session.status] ?? 'neutral'">{{ statusText }}</span>
        <template v-if="place"> · {{ place }}</template>
      </p>
      <p v-else-if="summary" class="sub" data-testid="phone-header-summary">{{ summary }}</p>
    </div>
    <UiButton type="button" class="icon" :aria-pressed="paletteOpen" :aria-label="t('mobile.commands')" :title="t('mobile.commands')" data-testid="phone-header-commands" @click="paletteOpen = !paletteOpen"><Command :size="18" /></UiButton>
    <UiButton v-if="canCreate" type="button" class="icon round" :aria-label="t('agents.newSession.action')" :title="t('agents.newSession.action')" data-testid="phone-header-new" @click="startNewSession()"><Plus :size="20" /></UiButton>
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

.status-icon {
  margin-right: 0.35em;
  vertical-align: -0.15em;
}

.status[data-tone='running'] {
  color: var(--vr-status-running);
}

.status[data-tone='feedback'] {
  color: var(--vr-status-feedback);
}

.status[data-tone='crashed'] {
  color: var(--vr-status-crashed);
}

.status[data-tone='limit'] {
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
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.icon[aria-pressed='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
}

.back {
  margin-left: calc(var(--vr-space-2) * -1);
  color: var(--vr-color-text);
}

.round {
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-full);
}
</style>
