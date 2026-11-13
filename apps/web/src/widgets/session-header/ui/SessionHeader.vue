<script setup lang="ts">
import { computed, watch } from 'vue';
import type { DockviewPanelApi } from 'dockview-vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { sessionDot, StatusIcon, useBerth, useExternalSessions, type AgentKind, type SessionStatus } from '../../../entities/session';
import { knownMr, lookupMr, numberLabel } from '../../../entities/merge-request';
import { SessionViewSwitch } from '../../../features/session-view-switch';

const props = defineProps<{ panel: DockviewPanelApi; active: boolean }>();

const KIND_LABEL: Record<AgentKind, string> = { claude: 'Claude Code', codex: 'Codex', cursor: 'Cursor', opencode: 'OpenCode' };

const berth = useBerth();
const external = useExternalSessions();

const node = computed(() => berth?.sessions.value.get(props.panel.id));
const entities = computed(() => berth?.entities.value);

const agent = computed(() => {
  const ref = node.value?.ref;
  return ref && 'Agent' in ref ? entities.value?.agents.get(ref.Agent) : undefined;
});
const worktree = computed(() => {
  const ref = node.value?.ref;
  const worktreeId = ref && 'Agent' in ref ? agent.value?.worktree_id : ref && 'Terminal' in ref ? entities.value?.terminals.get(ref.Terminal)?.worktree_id : undefined;
  return worktreeId ? entities.value?.worktrees.get(worktreeId) : undefined;
});
const project = computed(() => (worktree.value ? entities.value?.projects.get(worktree.value.project_id)?.name : undefined));
const mr = computed(() => (worktree.value ? knownMr(worktree.value.path, worktree.value.branch) : null));

const status = computed<SessionStatus | undefined>(() => {
  const session = node.value;
  return session ? sessionDot(session, external?.linked.value.get(props.panel.id)) : undefined;
});
const tone = computed(() => {
  if (status.value === 'finished' && node.value?.unseen) return 'unseen';
  if (status.value === 'running' || status.value === 'needs_feedback' || status.value === 'terminated') return { running: 'running', needs_feedback: 'feedback', terminated: 'crashed' }[status.value];
  return 'neutral';
});

const place = computed(() => [project.value, worktree.value?.branch].filter(Boolean).join(' › '));
const meta = computed(() =>
  [agent.value ? KIND_LABEL[agent.value.kind] : undefined, agent.value?.model, place.value, mr.value ? numberLabel(mr.value) : undefined].filter(Boolean).join(' · '),
);

watch(
  () => [worktree.value?.path, worktree.value?.branch] as const,
  ([path, branch]) => {
    if (path && branch) void lookupMr(path, branch);
  },
  { immediate: true },
);
</script>

<template>
  <header v-if="node" class="session-header" data-testid="session-header">
    <div class="identity">
      <div class="title-row">
        <span class="title" data-testid="session-header-title">{{ node.name }}</span>
        <span v-if="status" class="pill" :data-tone="tone" data-testid="session-header-status">
          <StatusIcon class="icon" :status="status" :unseen="node.unseen" />
          {{ t(`agents.status.${status}`) }}
        </span>
      </div>
      <div v-if="meta" class="meta" data-testid="session-header-meta">{{ meta }}</div>
    </div>
    <SessionViewSwitch :panel="panel" :active="active" />
  </header>
</template>

<style scoped>
.session-header {
  display: flex;
  flex: none;
  align-items: center;
  gap: 14px;
  min-height: 56px;
  padding: var(--vr-space-2) 14px var(--vr-space-2) 18px;
  border-bottom: 1px solid var(--vr-color-border);
}

.identity {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.title-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.title {
  overflow: hidden;
  font-size: 15px;
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.pill {
  display: flex;
  flex: none;
  align-items: center;
  gap: 5px;
  height: 22px;
  padding: 0 8px;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-full);
}

.icon {
  width: 12px;
  height: 12px;
}

.pill[data-tone='running'] {
  color: var(--vr-status-running);
  background: var(--vr-tint-running);
}

.pill[data-tone='feedback'] {
  color: var(--vr-status-feedback);
  background: var(--vr-tint-feedback);
}

.pill[data-tone='unseen'] {
  color: var(--vr-status-unseen);
  background: var(--vr-tint-unseen);
}

.pill[data-tone='crashed'] {
  color: var(--vr-status-crashed);
  background: var(--vr-tint-crashed);
}

.meta {
  overflow: hidden;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
