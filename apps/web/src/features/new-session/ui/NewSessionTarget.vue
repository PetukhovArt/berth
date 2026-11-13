<script setup lang="ts">
import { computed } from 'vue';
import { VInputText, VMessage, VSelect } from '../../../shared/ui/kit';
import { t } from '../../../shared/lib/i18n/i18n';
import { useBerth } from '../../../entities/session';
import type { BranchMode, SessionTarget } from '../model/sessionTarget';

const props = defineProps<{ state: SessionTarget }>();
const berth = useBerth();
const { project, target, mode, existing, fresh, branches, branchesError, loading, count, sharesBranch, projects, targets, creating, plan } = props.state;

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
  if (count.value > 1 && !sharesBranch.value) return t('agents.newSession.planCreateEach', { branch: current.branch });
  if (current.fetch && current.base) return t('agents.newSession.planCreateFresh', { branch: current.branch, base: current.base });
  return current.base ? t('agents.newSession.planCreateFrom', { branch: current.branch, base: current.base }) : t('agents.newSession.planCreate', { branch: current.branch });
});
</script>

<template>
  <div class="target" data-testid="new-session-target">
    <div class="chips">
      <VSelect v-model="project" :options="projects" option-label="label" option-value="id" size="small" class="chip" :aria-label="t('agents.newSession.project')" data-testid="new-session-project" />
      <VSelect v-model="target" :options="targets" option-label="label" option-value="id" size="small" class="chip" :aria-label="t('agents.newSession.target')" data-testid="new-session-worktree" />
      <VSelect v-if="!creating" v-model="mode" :options="MODES" option-label="label" option-value="value" option-disabled="disabled" size="small" class="chip" :aria-label="t('agents.newSession.branch')" data-testid="new-session-branch-mode" />
      <slot name="type" />
    </div>
    <label v-if="!creating && mode === 'existing'" class="row">
      <span class="row-label">{{ t('agents.newSession.existingBranch') }}</span>
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
        class="row-input"
        data-testid="new-session-existing-branch"
      />
    </label>
    <label v-if="creating || mode === 'new'" class="row">
      <span class="row-label">{{ t('agents.newSession.newBranch') }}</span>
      <VInputText v-model="fresh" size="small" :placeholder="t('agents.newSession.typeBranch')" class="row-input mono" data-testid="new-session-new-branch" />
    </label>
    <VMessage v-if="branchesError" severity="warn" size="small" data-testid="new-session-branches-error">{{ t('agents.newSession.branchesFailed', { error: branchesError }) }}</VMessage>
    <div class="summary" :data-plan="plan.kind" data-testid="new-session-plan">
      <svg class="arrow" width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 8h10M9 4l4 4-4 4" /></svg>
      <span>{{ summary }}</span>
    </div>
  </div>
</template>

<style scoped>
.target {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chip {
  height: 28px;
  min-width: 0;
  border: 0;
  border-radius: 7px;
  background: var(--vr-color-surface-muted);
  box-shadow: none;
  font-size: 12.5px;
}

.chip :deep(.p-select-label) {
  padding: 0 9px;
  white-space: nowrap;
}

.row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.row-label {
  width: 90px;
  flex: none;
  font-size: 12px;
  color: var(--vr-color-text-muted);
}

.row-input {
  flex: 1;
  min-width: 0;
}

.mono {
  font-family: var(--vr-font-mono);
  font-size: 12.5px;
}

.summary {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 12.5px;
  line-height: 18px;
  color: var(--vr-color-text-muted);
}

.arrow {
  flex: none;
  margin-top: 2px;
  color: currentColor;
}
</style>
