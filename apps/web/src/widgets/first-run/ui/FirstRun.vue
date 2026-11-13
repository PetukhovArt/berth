<script setup lang="ts">
import { computed, onMounted } from 'vue';
import { t } from '../../../shared/lib/i18n/i18n';
import { useAiAccounts, type AiStatus, type AiView } from '../../../shared/api/connectors/ai';
import { ProjectActions } from '../../../features/manage-workspace';
import { useBerth } from '../../../entities/session';

const STATUS = {
  signed_in: 'connectors.ai.status.signedIn',
  signed_out: 'connectors.ai.status.signedOut',
  not_installed: 'connectors.ai.status.notInstalled',
  rejected: 'connectors.ai.status.rejected',
  unreachable: 'connectors.ai.status.unreachable',
  error: 'connectors.ai.status.error',
  unchecked: 'connectors.ai.status.unchecked',
} as const satisfies Record<AiStatus, string>;

const berth = useBerth();
const { accounts, refresh } = useAiAccounts();

const cli = computed(() => accounts.value.filter((a) => a.method === 'cli'));
const hasSignedIn = computed(() => cli.value.some((a) => a.status === 'signed_in'));
const workspaceId = computed(() => berth?.tree.value[0]?.id);

const tone = (status: AiStatus) => (status === 'signed_in' ? 'ok' : status === 'rejected' || status === 'error' ? 'error' : 'idle');
const state = (a: AiView) => (a.status === 'signed_in' && (a.account || a.plan) ? [a.account, a.plan].filter(Boolean).join(' · ') : t(STATUS[a.status]));

onMounted(() => {
  void refresh();
});
</script>

<template>
  <section class="first-run" data-testid="first-run">
    <header class="head">
      <h1 class="heading" data-testid="first-run-title">{{ t('workbench.firstRun.title') }}</h1>
      <p class="lead" data-testid="first-run-lead">{{ t('workbench.firstRun.lead') }}</p>
    </header>
    <div class="block">
      <h2 class="title">{{ t('connectors.ai.title') }}</h2>
      <ul class="agents" data-testid="first-run-agents">
        <li v-for="a in cli" :key="a.kind" class="agent" :data-status="a.status" :data-testid="`first-run-agent-${a.kind}`">
          <span class="name">{{ a.label }}</span>
          <span class="state" :data-tone="tone(a.status)">{{ state(a) }}</span>
        </li>
      </ul>
    </div>
    <div class="block">
      <ProjectActions v-if="workspaceId" class="add" :workspace-id="workspaceId" primary />
      <p v-if="!hasSignedIn" class="next" data-testid="first-run-next">{{ t('workbench.firstRun.nextStep') }}</p>
    </div>
  </section>
</template>

<style scoped>
.first-run {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-6);
  width: min(100%, 36rem);
  margin: auto;
  padding: var(--vr-space-8) var(--vr-space-4);
}

.head {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.heading {
  margin: 0;
  font-size: var(--vr-font-size-display);
  font-weight: 600;
  color: var(--vr-color-text);
}

.lead {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.next {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-subtle);
}

.block {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.title {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
  color: var(--vr-color-text-muted);
}

.agents {
  margin: 0;
  padding: 0;
  list-style: none;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
  background: var(--vr-color-surface);
}

.agent {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--vr-space-3);
  padding: var(--vr-space-3) var(--vr-space-4);
}

.agent + .agent {
  border-top: 1px solid var(--vr-color-border);
}

.name {
  color: var(--vr-color-text);
}

.state {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.state[data-tone='ok'] {
  color: var(--vr-color-success);
}

.state[data-tone='error'] {
  color: var(--vr-color-danger);
}

.add {
  justify-content: flex-start;
}
</style>
