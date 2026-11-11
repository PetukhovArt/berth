<script setup lang="ts">
import { computed } from 'vue';
import { VInputText, VMessage, VSelect } from '@ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { useBerth } from '../../../entities/session/model/berth';
import type { BranchMode, SessionTarget } from '../model/sessionTarget';

const props = defineProps<{ state: SessionTarget }>();
const berth = useBerth();
const { project, target, mode, existing, fresh, branches, branchesError, loading, projects, targets, creating, plan } = props.state;

const MODES = computed<{ label: string; value: BranchMode; disabled?: boolean }[]>(() => [
  { label: t('agents.newSession.branchCurrent'), value: 'current' },
  { label: t('agents.newSession.branchExisting'), value: 'existing', disabled: !branches.value },
  { label: t('agents.newSession.branchNew'), value: 'new' },
]);

const branchOptions = computed(() =>
  (branches.value ?? []).map((branch) => {
    const holder = branch.worktree ? berth?.entities.value.worktrees.get(branch.worktree) : undefined;
    const where = holder ? (holder.is_main ? t('agents.newSession.inMain') : t('agents.newSession.inWorktree')) : t('agents.newSession.noCheckout');
    return { label: `${branch.name} · ${where}`, value: branch.name };
  }),
);

const summary = computed(() => {
  const current = plan.value;
  if (current.kind === 'incomplete') return current.reason;
  if (current.kind === 'use') return t(current.worktree.is_main ? 'agents.newSession.planMain' : 'agents.newSession.planUse', { branch: current.worktree.branch });
  if (current.fetch && current.base) return t('agents.newSession.planCreateFresh', { branch: current.branch, base: current.base });
  return current.base ? t('agents.newSession.planCreateFrom', { branch: current.branch, base: current.base }) : t('agents.newSession.planCreate', { branch: current.branch });
});
</script>

<template>
  <div class="target" data-testid="new-session-target">
    <div class="pair">
      <label class="field">
        <span>{{ t('agents.newSession.project') }}</span>
        <VSelect v-model="project" :options="projects" option-label="label" option-value="id" size="small" :aria-label="t('agents.newSession.project')" data-testid="new-session-project" />
      </label>
      <label class="field">
        <span>{{ t('agents.newSession.target') }}</span>
        <VSelect v-model="target" :options="targets" option-label="label" option-value="id" size="small" :aria-label="t('agents.newSession.target')" data-testid="new-session-worktree" />
      </label>
    </div>
    <label v-if="!creating" class="field">
      <span>{{ t('agents.newSession.branch') }}</span>
      <VSelect v-model="mode" :options="MODES" option-label="label" option-value="value" option-disabled="disabled" size="small" :aria-label="t('agents.newSession.branch')" data-testid="new-session-branch-mode" />
    </label>
    <label v-if="!creating && mode === 'existing'" class="field">
      <span>{{ t('agents.newSession.existingBranch') }}</span>
      <VSelect
        v-model="existing"
        :options="branchOptions"
        option-label="label"
        option-value="value"
        filter
        size="small"
        :loading="loading"
        :placeholder="t('agents.newSession.pickBranch')"
        :aria-label="t('agents.newSession.existingBranch')"
        data-testid="new-session-existing-branch"
      />
    </label>
    <label v-if="creating || mode === 'new'" class="field">
      <span>{{ t('agents.newSession.newBranch') }}</span>
      <VInputText v-model="fresh" size="small" :placeholder="t('agents.newSession.typeBranch')" data-testid="new-session-new-branch" />
    </label>
    <VMessage v-if="branchesError" severity="warn" size="small" data-testid="new-session-branches-error">{{ t('agents.newSession.branchesFailed', { error: branchesError }) }}</VMessage>
    <div class="summary" :data-plan="plan.kind" data-testid="new-session-plan">{{ summary }}</div>
  </div>
</template>

<style scoped>
.target {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.field {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--vr-space-1);
  min-width: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.pair {
  display: flex;
  gap: var(--vr-space-3);
}

.summary {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}
</style>
